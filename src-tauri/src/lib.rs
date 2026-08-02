pub mod commands;
pub mod config;
pub mod db;
pub mod error;
pub mod fs;
pub mod jobs;
pub mod media;
pub mod sidecar;

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, MutexGuard, RwLock};

use rusqlite::Connection;
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::config::{Config, WindowState};
use crate::error::{AppError, Result};
use crate::fs::paths::LibraryPaths;
use crate::fs::watch::{Suppressor, Watch};
use crate::jobs::JobQueue;
use crate::sidecar::Tools;

/// Point WebView2's profile inside the app directory. Called from `main`
/// before Tauri starts; `build_window` sets the same path on the window
/// builder, because the env var alone is only honoured when nothing else
/// specifies one.
pub fn redirect_webview_data_dir() {
    if let Ok(dir) = config::webview_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &dir);
    }
}

/// An open library: its paths, its database, and the workers chewing through
/// its job queue.
pub struct Library {
    pub paths: LibraryPaths,
    pub tools: Tools,
    conn: Mutex<Connection>,
    queue: JobQueue,
    /// `None` once `close` has stopped it. Behind a mutex rather than held
    /// directly so `close(&self)` — called through a shared `Arc<Library>` —
    /// can take ownership of it and call its `&mut self` `stop`.
    watch: Mutex<Option<Watch>>,
    _lock: LockFile,
}

impl Library {
    pub fn open(app: AppHandle, root: PathBuf) -> Result<Library> {
        if !root.is_dir() {
            return Err(AppError::invalid(format!(
                "{} is not a folder",
                root.display()
            )));
        }

        let paths = LibraryPaths::new(root);
        paths.ensure_dirs()?;

        // Single instance per library, per DESIGN.md. Two copies of the app
        // sharing one database would each run a full job queue against it.
        let lock = LockFile::acquire(&paths.lock_path())?;

        let mut conn = db::open(&paths.db_path())?;
        db::migrate(&mut conn)?;
        // Jobs abandoned by a crash go back in the queue.
        db::jobs::requeue_running(&conn)?;

        // Only worth reading while a library is still mid-first-import: once
        // `imported_at` is set, every row the walker can still create fresh
        // already carries its real name, so the lookup would only ever miss.
        let rename_lookup = if db::settings::imported_at(&conn)?.is_none() {
            fs::import::load_rename_lookup(&paths)
        } else {
            Default::default()
        };

        // The webview may read thumbnails and sprites, and nothing else. The
        // library's own media is not exposed until a viewer needs it (M2).
        let _ = app
            .asset_protocol_scope()
            .allow_directory(paths.cache_dir(), true);

        let tools = Tools::discover();
        // Shared between the job queue and the filesystem watcher: the
        // watcher sets `rescanning` before queuing a reconcile walk and
        // suppresses its own renames, the queue's `Progress` reads the
        // former back and its hash job consults the latter. See
        // `fs::watch`.
        let suppressor = Suppressor::default();
        let rescanning: fs::watch::Rescanning = Arc::new(AtomicBool::new(false));
        let queue = JobQueue::start(
            app,
            paths.clone(),
            tools.clone(),
            paths.db_path(),
            rename_lookup,
            suppressor.clone(),
            Arc::clone(&rescanning),
        )?;
        let watch = fs::watch::start(paths.clone(), paths.db_path(), suppressor, rescanning)?;

        Ok(Library {
            paths,
            tools,
            conn: Mutex::new(conn),
            queue,
            watch: Mutex::new(Some(watch)),
            _lock: lock,
        })
    }

    pub fn conn(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::invalid("database connection is poisoned"))
    }

    pub fn queue(&self) -> &JobQueue {
        &self.queue
    }

    /// Stop the watcher and the workers and collapse the WAL, so a closed
    /// library is a single `.db` file that can simply be copied.
    pub fn close(&self) {
        if let Ok(mut guard) = self.watch.lock() {
            if let Some(mut watch) = guard.take() {
                watch.stop();
            }
        }
        self.queue.stop();
        if let Ok(conn) = self.conn() {
            let _ = db::checkpoint(&conn);
        }
    }
}

/// `.gallery/lock`, held open with no sharing. The file's existence means
/// nothing; the exclusive handle is the lock, so a crash releases it.
struct LockFile {
    _file: std::fs::File,
}

impl LockFile {
    fn acquire(path: &Path) -> Result<LockFile> {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            options.share_mode(0);
        }

        match options.open(path) {
            Ok(file) => Ok(LockFile { _file: file }),
            // 32 is ERROR_SHARING_VIOLATION: another instance holds it.
            Err(err) if err.raw_os_error() == Some(32) => Err(AppError::AlreadyOpen),
            Err(err) => Err(AppError::Io(err)),
        }
    }
}

#[derive(Default)]
pub struct AppState {
    library: RwLock<Option<Arc<Library>>>,
    /// The M1.7 startup flow's scan result, held between `prepare_import` and
    /// `execute_prepared_import` — there is no `Library` yet at this point,
    /// so it cannot live alongside one. See `fs::import::PendingImport`.
    pending_import: Mutex<Option<fs::import::PendingImport>>,
}

impl AppState {
    pub fn library(&self) -> Result<Arc<Library>> {
        self.library
            .read()
            .map_err(|_| AppError::invalid("library state is poisoned"))?
            .clone()
            .ok_or(AppError::NoLibrary)
    }

    pub fn current(&self) -> Option<Arc<Library>> {
        self.library.read().ok().and_then(|lib| lib.clone())
    }

    pub fn set(&self, library: Arc<Library>) -> Result<()> {
        let mut slot = self
            .library
            .write()
            .map_err(|_| AppError::invalid("library state is poisoned"))?;
        if let Some(previous) = slot.take() {
            previous.close();
        }
        *slot = Some(library);
        Ok(())
    }

    pub fn take(&self) -> Option<Arc<Library>> {
        self.library.write().ok().and_then(|mut slot| slot.take())
    }

    pub fn set_pending_import(&self, pending: fs::import::PendingImport) -> Result<()> {
        let mut slot = self
            .pending_import
            .lock()
            .map_err(|_| AppError::invalid("pending import state is poisoned"))?;
        *slot = Some(pending);
        Ok(())
    }

    /// Takes the plan rather than cloning it — a plan is only ever consumed
    /// once, by the execute step that follows the review it was built for.
    pub fn take_pending_import(&self) -> Result<Option<fs::import::PendingImport>> {
        Ok(self
            .pending_import
            .lock()
            .map_err(|_| AppError::invalid("pending import state is poisoned"))?
            .take())
    }

    pub fn clear_pending_import(&self) -> Result<()> {
        self.take_pending_import()?;
        Ok(())
    }
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::library::open_library,
            commands::library::current_library,
            commands::library::close_library,
            commands::library::folder_tree,
            commands::items::list_items,
            commands::jobs::start_index,
            commands::jobs::index_progress,
            commands::jobs::index_failures,
            commands::jobs::retry_failed_jobs,
            commands::import::scan_import,
            commands::import::dry_run_import,
            commands::import::execute_import,
            commands::import::verify_import,
            commands::import::mark_imported,
            commands::import::prepare_import,
            commands::import::execute_prepared_import,
            commands::import::cancel_prepared_import,
            commands::folders::get_folder,
            commands::folders::set_folder_title,
            commands::folders::set_folder_status,
            commands::folders::set_folder_favorite,
            commands::folders::set_folder_notes,
            commands::folders::apply_folder_archetype,
            commands::folders::set_folder_label,
            commands::folders::add_folder_flag,
            commands::folders::remove_folder_tag,
            commands::folders::list_folder_statuses,
            commands::folders::list_archetypes,
            commands::folders::create_folder,
            commands::folders::rename_folder_dir,
            commands::folders::move_folder,
            commands::folders::delete_folder,
            commands::folders::create_archetype,
            commands::folders::rename_archetype,
            commands::folders::delete_archetype,
            commands::folders::count_folders_using_archetype,
            commands::folders::add_archetype_field,
            commands::folders::reorder_archetype_fields,
            commands::folders::archetype_field_usage,
            commands::folders::remove_archetype_field,
            commands::folders::create_folder_status,
            commands::folders::rename_folder_status,
            commands::folders::recolour_folder_status,
            commands::folders::reorder_folder_statuses,
            commands::folders::count_folders_by_status,
            commands::folders::remove_folder_status,
            commands::items::move_items,
            commands::items::delete_items,
            commands::items::reveal_item,
            commands::items::open_item,
            commands::items::copy_item_file,
            commands::items::copy_item_path,
            commands::tags::item_effective_tags,
            commands::tags::add_item_tag,
            commands::tags::remove_item_tag,
            commands::tags::list_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
        ])
        .setup(|app| {
            build_window(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to start the application");

    app.run(|handle, event| match event {
        // Geometry has to be read while the window still exists; by the time
        // `Exit` arrives it has been destroyed.
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { .. },
            ..
        } => save_window_state(handle),
        tauri::RunEvent::Exit => shutdown(handle),
        _ => {}
    });
}

/// The window is built here rather than declared in `tauri.conf.json` because
/// `data_directory` — the WebView2 redirect — has no configuration-file
/// equivalent.
fn build_window(app: &AppHandle) -> Result<()> {
    let config = Config::load();

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("GGallery")
        // Conservative: a logical size that still fits a 1080p screen once
        // Windows applies display scaling.
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 600.0);

    #[cfg(windows)]
    {
        builder = builder.data_directory(config::webview_data_dir()?);
    }

    if let Some(state) = config.window {
        builder = builder
            .inner_size(state.width as f64, state.height as f64)
            .position(state.x as f64, state.y as f64);
    }

    let window = builder.build()?;
    if config.window.map(|state| state.maximized).unwrap_or(false) {
        let _ = window.maximize();
    }

    #[cfg(debug_assertions)]
    window.open_devtools();

    Ok(())
}

fn save_window_state(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let (Ok(size), Ok(position), Ok(maximized), Ok(scale)) = (
        window.inner_size(),
        window.outer_position(),
        window.is_maximized(),
        window.scale_factor(),
    ) else {
        return;
    };

    // Geometry comes back in physical pixels; the builder takes logical ones.
    // Storing physical would grow the window by the scale factor every launch.
    let size = size.to_logical::<f64>(scale);
    let position = position.to_logical::<f64>(scale);

    let _ = Config::set_window(WindowState {
        width: size.width.round() as u32,
        height: size.height.round() as u32,
        x: position.x.round() as i32,
        y: position.y.round() as i32,
        maximized,
    });
}

/// Stop the workers and leave the library as a single `.db` file.
fn shutdown(app: &AppHandle) {
    if let Some(library) = app.state::<AppState>().take() {
        library.close();
    }
}
