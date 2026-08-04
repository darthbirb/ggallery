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

        // Thumbnails and sprites for the grid, and — from M2.5a, which builds
        // the pane's Preview mode — the originals themselves, which the
        // viewer displays at full size. Read access to a local folder the
        // user chose, granted to a webview that loads no remote content.
        let _ = app
            .asset_protocol_scope()
            .allow_directory(paths.cache_dir(), true);
        let _ = app
            .asset_protocol_scope()
            .allow_directory(paths.root(), true);

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
            commands::library::ui_prefs,
            commands::library::set_ui_prefs,
            commands::items::list_items,
            commands::items::get_item,
            commands::items::set_items_favorite,
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
            commands::folders::set_folder_cover,
            commands::folders::reveal_folder,
            commands::folders::apply_folder_archetype,
            commands::folders::remove_folder_archetype,
            commands::folders::set_folder_label,
            commands::folders::add_folder_flag,
            commands::folders::remove_folder_tag,
            commands::folders::list_folder_statuses,
            commands::folders::list_archetypes,
            commands::folders::create_folder,
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
            commands::tags::folder_inherited_tags,
            commands::tags::add_item_tag,
            commands::tags::remove_item_tag,
            commands::tags::list_tags,
            commands::tags::rename_tag,
            commands::tags::delete_tag,
            commands::triage::undo_batch,
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

const MIN_WINDOW_WIDTH: f64 = 960.0;
const MIN_WINDOW_HEIGHT: f64 = 600.0;

/// 70% of the primary monitor's work area, centred within it. `None` when no
/// monitor can be identified — `build_window` falls back to a fixed size.
///
/// Monitor geometry comes back in physical pixels; the builder's
/// `inner_size`/`position` take logical ones, so everything here is divided
/// by the scale factor before use, matching `save_window_state`'s conversion
/// the other way.
fn default_window_geometry(app: &AppHandle) -> Option<(f64, f64, f64, f64)> {
    let monitor = app.primary_monitor().ok().flatten()?;
    let scale = monitor.scale_factor();
    let work_area = monitor.work_area();

    let area_width = work_area.size.width as f64 / scale;
    let area_height = work_area.size.height as f64 / scale;
    let area_x = work_area.position.x as f64 / scale;
    let area_y = work_area.position.y as f64 / scale;

    let width = (area_width * 0.7).max(MIN_WINDOW_WIDTH);
    let height = (area_height * 0.7).max(MIN_WINDOW_HEIGHT);
    let x = area_x + (area_width - width) / 2.0;
    let y = area_y + (area_height - height) / 2.0;

    Some((width, height, x, y))
}

/// The window is built here rather than declared in `tauri.conf.json` because
/// `data_directory` — the WebView2 redirect — has no configuration-file
/// equivalent.
fn build_window(app: &AppHandle) -> Result<()> {
    let config = Config::load();

    // `bundle.icon` (tauri.conf.json) embeds this into the exe's own Windows
    // resource at build time, via `tauri-build`; that covers Explorer's file
    // icon. It says nothing about the icon a *running* window carries — that
    // has to be set on the builder, or the window falls back to a default
    // and the taskbar shows an upscaled version of it.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;

    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::default())
        .title("GGallery")
        .icon(icon)?
        .min_inner_size(MIN_WINDOW_WIDTH, MIN_WINDOW_HEIGHT)
        // Decision 28: the window bar is ours, not Windows'. Snap Layouts'
        // flyout is knowingly given up — it only appears over a native
        // maximise button — but edge-drag resizing and edge-snap are
        // unaffected; see docs/DESIGN.md §2.
        .decorations(false)
        // Windows' DWM computes the minimize/restore genie-effect animation
        // from the window's frame, which an undecorated window has none of —
        // a long-standing upstream Tauri/Windows gap (tauri-apps/tauri#2064)
        // that shows up as minimize/restore snapping to the wrong bounds
        // with no animation. `shadow(true)` gives Windows 11 a real DWM
        // frame to extend (a 1px border, rounded corners) without bringing
        // back the title bar; worth confirming interactively since the
        // same combination has its own reported off-by-a-pixel sizing case
        // (tauri-apps/tauri#12285) — not a guaranteed fix, the best
        // available lever.
        .shadow(true)
        // Kept invisible until the decorations toggle below has run, so the
        // trick never flashes a titlebar into view on the way up.
        .visible(false);

    #[cfg(windows)]
    {
        builder = builder.data_directory(config::webview_data_dir()?);
    }

    if let Some(state) = config.window {
        builder = builder
            .inner_size(state.width as f64, state.height as f64)
            .position(state.x as f64, state.y as f64);
    } else if let Some((width, height, x, y)) = default_window_geometry(app) {
        builder = builder.inner_size(width, height).position(x, y);
    } else {
        // No monitor info available — fall back to a conservative logical
        // size that still fits a 1080p screen once Windows applies scaling.
        builder = builder.inner_size(1280.0, 820.0);
    }

    let window = builder.build()?;
    if config.window.map(|state| state.maximized).unwrap_or(false) {
        let _ = window.maximize();
    }

    // `shadow(true)` alone still leaves minimize/restore snapping to the
    // wrong bounds on some machines — Windows appears to only wire a window
    // into DWM's genie-effect animation when it is first shown carrying a
    // frame, and building directly with `decorations(false)` can skip that
    // registration entirely. Toggling decorations on and back off forces a
    // `SWP_FRAMECHANGED` recalculation that re-registers it, done here while
    // the window is still hidden so nothing flashes.
    let _ = window.set_decorations(true);
    let _ = window.set_decorations(false);
    let _ = window.show();

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
