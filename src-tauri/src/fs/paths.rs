//! The only place in the application that converts between absolute paths and
//! library-relative ones.
//!
//! Everything stored in the database is relative to the library root, uses
//! forward slashes, and is case-normalised. No absolute path ever reaches a
//! table. If path handling appears anywhere else, that is a bug even if it
//! happens to work.

use std::path::{Component, Path, PathBuf};

use crate::error::{AppError, Result};

/// Name of the app-owned directory inside the library root. The only place
/// M1 is allowed to write.
pub const GALLERY_DIR: &str = ".gallery";

#[derive(Debug, Clone)]
pub struct LibraryPaths {
    root: PathBuf,
}

impl LibraryPaths {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        LibraryPaths { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn gallery_dir(&self) -> PathBuf {
        self.root.join(GALLERY_DIR)
    }

    pub fn db_path(&self) -> PathBuf {
        self.gallery_dir().join("library.db")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.gallery_dir().join("lock")
    }

    pub fn jsonl_path(&self) -> PathBuf {
        self.gallery_dir().join("library.jsonl")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.gallery_dir().join("cache")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache_dir().join("thumbs")
    }

    pub fn sprites_dir(&self) -> PathBuf {
        self.cache_dir().join("sprites")
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.gallery_dir().join("trash")
    }

    pub fn pending_dir(&self) -> PathBuf {
        self.gallery_dir().join("pending")
    }

    /// Create the app-owned directory tree. Touches nothing else in the root.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            self.gallery_dir(),
            self.cache_dir(),
            self.thumbs_dir(),
            self.sprites_dir(),
            self.trash_dir(),
            self.pending_dir(),
        ] {
            std::fs::create_dir_all(dir)?;
        }
        Ok(())
    }

    /// Absolute path → library-relative, normalised. Fails if the path is not
    /// inside the root, which is what stops anything outside it being indexed.
    pub fn to_rel(&self, abs: &Path) -> Result<String> {
        let rel = abs
            .strip_prefix(&self.root)
            .map_err(|_| AppError::OutsideLibrary(abs.to_string_lossy().to_string()))?;
        Ok(normalise_rel(&rel.to_string_lossy()))
    }

    /// Library-relative → absolute. Rejects anything trying to climb out.
    pub fn to_abs(&self, rel: &str) -> Result<PathBuf> {
        let rel = normalise_rel(rel);
        if rel.is_empty() {
            return Ok(self.root.clone());
        }
        let candidate = PathBuf::from(&rel);
        if candidate
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(AppError::OutsideLibrary(rel));
        }
        Ok(self.root.join(candidate))
    }

    /// Absolute path of the file backing an item.
    ///
    /// `disk_name` is whatever the user already had, until `fs::import`
    /// renames it to `<uuid>.<ext>` — this function does not care which, it
    /// just joins the folder and whatever name the row currently holds.
    pub fn item_path(&self, folder_rel: &str, disk_name: &str) -> Result<PathBuf> {
        Ok(self.to_abs(folder_rel)?.join(disk_name))
    }

    pub fn thumb_path(&self, uuid: &str) -> PathBuf {
        self.thumbs_dir().join(shard(uuid))
    }

    pub fn sprite_path(&self, uuid: &str) -> PathBuf {
        self.sprites_dir().join(shard(uuid))
    }

    /// True for anything under `<root>/.gallery` — the app's own storage,
    /// never indexed as library content.
    pub fn is_gallery_dir(&self, abs: &Path) -> bool {
        abs.starts_with(self.gallery_dir())
    }
}

/// `<uuid>` → `ab/cd/<uuid>.webp`. Two levels of 256-way sharding keeps any one
/// directory under a few hundred entries at 100k items — Windows explorer and
/// NTFS both dislike flat directories at that size.
pub fn shard(uuid: &str) -> String {
    let clean: String = uuid.chars().filter(|c| c.is_ascii_alphanumeric()).collect();
    let a = clean.get(0..2).unwrap_or("00");
    let b = clean.get(2..4).unwrap_or("00");
    format!("{a}/{b}/{uuid}.webp")
}

/// Forward slashes, no leading or trailing separator, lower case.
///
/// Case normalisation is what makes a library survive being copied between
/// machines and filesystems; display names live in `folder.title` and
/// `item.orig_name`, which keep their original case.
pub fn normalise_rel(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|seg| !seg.is_empty() && *seg != ".")
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

/// Whether two absolute paths name the same directory.
///
/// Windows paths are case-insensitive and tolerate either separator, so a
/// plain `==` would decide that `D:\Media` and `D:/media\` are different
/// libraries — and then refuse to reopen the one already open.
pub fn same_dir(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        return a == b;
    }
    fn key(path: &Path) -> String {
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_end_matches('/')
            .to_lowercase()
    }
    key(a) == key(b)
}

/// Parent of a relative path, or `None` for the root folder itself.
pub fn parent_rel(rel: &str) -> Option<String> {
    if rel.is_empty() {
        return None;
    }
    match rel.rsplit_once('/') {
        Some((parent, _)) => Some(parent.to_string()),
        None => Some(String::new()),
    }
}

/// Lowercase extension without the dot.
pub fn extension_of(name: &str) -> String {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_separators_and_case() {
        assert_eq!(normalise_rel("People\\Ana\\"), "people/ana");
        assert_eq!(normalise_rel("/People//Ana"), "people/ana");
        assert_eq!(normalise_rel(""), "");
    }

    #[test]
    fn parents_walk_up_to_root() {
        assert_eq!(parent_rel("people/ana"), Some("people".into()));
        assert_eq!(parent_rel("people"), Some(String::new()));
        assert_eq!(parent_rel(""), None);
    }

    #[test]
    fn rejects_escaping_paths() {
        let paths = LibraryPaths::new("C:/lib");
        assert!(paths.to_abs("../outside").is_err());
    }

    #[test]
    fn shards_by_first_four_characters() {
        assert_eq!(shard("abcdef12"), "ab/cd/abcdef12.webp");
    }
}
