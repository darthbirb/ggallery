//! `gallery.config.json`, which lives next to the executable and nowhere else.
//!
//! Nothing in here may write outside the app directory. That is the rule that
//! lets the whole application run from a USB stick.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{AppError, Result};

const CONFIG_NAME: &str = "gallery.config.json";

/// Directory the executable sits in. In dev that is `src-tauri/target/debug`,
/// which is correct — config and the WebView2 profile belong next to whichever
/// binary is running.
pub fn app_dir() -> Result<PathBuf> {
    let exe = std::env::current_exe()?;
    exe.parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| AppError::invalid("executable has no parent directory"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(app_dir()?.join(CONFIG_NAME))
}

/// WebView2's user data folder, forced inside the app directory. Tauri would
/// otherwise put it in `%LOCALAPPDATA%\<bundle-id>\`.
pub fn webview_data_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("webview2"))
}

/// Where sidecar binaries are looked for.
pub fn tools_dir() -> Result<PathBuf> {
    Ok(app_dir()?.join("tools"))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    /// Absolute path to the library root. The only absolute path the app
    /// stores anywhere — deliberately outside the database.
    pub library_root: Option<String>,
    pub window: Option<WindowState>,
    /// Panel widths, the folded and expanded states, the accent, tile size —
    /// everything docs/DESIGN.md §2 says "persists between sessions alongside
    /// window geometry", and explicitly never in the database.
    ///
    /// Deliberately opaque here. The shape belongs to `src/state/ui.ts`, which
    /// gains a field or two in most later milestones; the backend's only job is
    /// to write it next to the exe and hand it back. Nothing in Rust reads
    /// inside it, so nothing in Rust has to change when it grows.
    pub ui: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowState {
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub maximized: bool,
}

impl Config {
    pub fn load() -> Config {
        let Ok(path) = config_path() else {
            return Config::default();
        };
        let Ok(text) = std::fs::read_to_string(path) else {
            return Config::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    pub fn set_library_root(root: &Path) -> Result<()> {
        let mut cfg = Config::load();
        cfg.library_root = Some(root.to_string_lossy().to_string());
        cfg.save()
    }

    pub fn set_window(state: WindowState) -> Result<()> {
        let mut cfg = Config::load();
        cfg.window = Some(state);
        cfg.save()
    }

    pub fn set_ui(prefs: serde_json::Value) -> Result<()> {
        let mut cfg = Config::load();
        cfg.ui = Some(prefs);
        cfg.save()
    }
}
