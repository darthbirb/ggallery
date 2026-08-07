//! Copies a file onto the Windows clipboard as a real file (`CF_HDROP`), so
//! pasting into Explorer, Discord, etc. produces the actual file — not its
//! path as text. This is the escape hatch SPEC.md's item operations
//! promise; a plain text path would defeat the purpose, since the whole
//! reason it's needed is that filenames on disk are opaque UUIDs.
//!
//! **Known limitation, not solved here**: the pasted file carries its
//! on-disk UUID name, not a reconstructed one. Fixing that means staging a
//! renamed copy before putting it on the clipboard, and that naming logic
//! belongs with M8's Export feature, which already owns filename
//! reconstruction — see SPEC.md's item-operations note.

use std::path::Path;

use clipboard_win::{raw, Clipboard};

use crate::error::{AppError, Result};

/// `raw::*` assumes an already-open clipboard — unlike the crate's generic
/// `set_clipboard` helper, which only supports `Setter` impls over a Sized
/// type and `FileList`'s is over `[T]` (unsized), so that helper cannot be
/// used here at all.
fn open() -> Result<Clipboard> {
    Clipboard::new_attempts(10)
        .map_err(|err| AppError::invalid(format!("couldn't open the clipboard: {err}")))
}

pub fn copy_file(path: &Path) -> Result<()> {
    let _clip = open()?;
    let path_str = path.to_string_lossy().to_string();
    raw::set_file_list(&[path_str])
        .map_err(|err| AppError::invalid(format!("couldn't copy the file: {err}")))
}

pub fn copy_text(text: &str) -> Result<()> {
    let _clip = open()?;
    raw::set_string(text).map_err(|err| AppError::invalid(format!("couldn't copy: {err}")))
}
