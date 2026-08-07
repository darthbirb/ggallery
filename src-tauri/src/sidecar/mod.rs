//! Sidecar binaries. **Nothing else in the application spawns a process.**
//!
//! M1 needs ffmpeg and ffprobe for video probing, poster frames and scrub
//! strips. The pinned-version download and checksum verification that DECISIONS.md
//! describes belong to the tool updater in M5; until then the app looks for
//! the binaries in `tools/` next to the executable, then falls back to PATH so
//! a machine with ffmpeg already installed works today. Reading a binary from
//! PATH writes nothing outside the app directory, so the portability rule
//! holds either way.

pub mod ffmpeg;

use std::path::{Path, PathBuf};

use crate::config;

#[derive(Debug, Clone, Default)]
pub struct Tools {
    pub ffmpeg: Option<ffmpeg::Ffmpeg>,
}

impl Tools {
    pub fn discover() -> Tools {
        let tools_dir = config::tools_dir().ok();
        let ffmpeg = locate("ffmpeg", tools_dir.as_deref());
        let ffprobe = locate("ffprobe", tools_dir.as_deref());
        Tools {
            ffmpeg: match (ffmpeg, ffprobe) {
                (Some(ffmpeg), Some(ffprobe)) => Some(ffmpeg::Ffmpeg::new(ffmpeg, ffprobe)),
                _ => None,
            },
        }
    }

    /// Human-readable summary for the library status panel, so "why are there
    /// no video thumbnails" is answerable without reading a log.
    pub fn describe(&self) -> Option<String> {
        self.ffmpeg
            .as_ref()
            .map(|f| f.binary().to_string_lossy().to_string())
    }
}

fn locate(stem: &str, tools_dir: Option<&Path>) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{stem}.exe")
    } else {
        stem.to_string()
    };

    if let Some(dir) = tools_dir {
        let candidate = dir.join(&file);
        if candidate.is_file() {
            return Some(candidate);
        }
    }

    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(&file))
        .find(|candidate| candidate.is_file())
}
