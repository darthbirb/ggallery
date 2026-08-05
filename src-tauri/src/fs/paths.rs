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
pub const GGALLERY_DIR: &str = ".ggallery";

/// The name this directory carried before the app had one of its own —
/// `gallery` was a placeholder. Referenced only by [`LibraryPaths::migrate_legacy_dir`],
/// the one-time rename that catches up a library created before this change.
const LEGACY_GALLERY_DIR: &str = ".gallery";

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

    pub fn ggallery_dir(&self) -> PathBuf {
        self.root.join(GGALLERY_DIR)
    }

    fn legacy_gallery_dir(&self) -> PathBuf {
        self.root.join(LEGACY_GALLERY_DIR)
    }

    /// Catches up a library created before the app-owned directory was named
    /// after the app: renames `.gallery/` to `.ggallery/` in one atomic,
    /// same-volume move. Must run before anything else touches the root —
    /// in particular before [`ensure_dirs`](Self::ensure_dirs), which would
    /// otherwise create a fresh, empty `.ggallery/` and strand the real one.
    ///
    /// A library carrying both directories is a state that should never
    /// exist — this refuses to guess which one is current rather than
    /// silently opening one and stranding the other's tags and folders.
    pub fn migrate_legacy_dir(&self) -> Result<()> {
        let legacy = self.legacy_gallery_dir();
        let current = self.ggallery_dir();
        match (legacy.is_dir(), current.is_dir()) {
            (true, true) => Err(AppError::invalid(format!(
                "{} has both {} and {} — remove or merge one by hand before opening it",
                self.root.display(),
                LEGACY_GALLERY_DIR,
                GGALLERY_DIR
            ))),
            (true, false) => {
                std::fs::rename(&legacy, &current)?;
                Ok(())
            }
            (false, _) => Ok(()),
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.ggallery_dir().join("library.db")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.ggallery_dir().join("lock")
    }

    pub fn jsonl_path(&self) -> PathBuf {
        self.ggallery_dir().join("library.jsonl")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.ggallery_dir().join("cache")
    }

    pub fn thumbs_dir(&self) -> PathBuf {
        self.cache_dir().join("thumbs")
    }

    pub fn sprites_dir(&self) -> PathBuf {
        self.cache_dir().join("sprites")
    }

    pub fn trash_dir(&self) -> PathBuf {
        self.ggallery_dir().join("trash")
    }

    pub fn pending_dir(&self) -> PathBuf {
        self.ggallery_dir().join("pending")
    }

    pub fn backups_dir(&self) -> PathBuf {
        self.ggallery_dir().join("backups")
    }

    /// Every file in the library, sharded by uuid (PLAN.md decision 30).
    /// Nothing outside `fs::shard` and this method should ever construct a
    /// path beneath it directly.
    pub fn files_dir(&self) -> PathBuf {
        self.root.join("files")
    }

    /// The only place on disk a user is meant to put files by hand — watched,
    /// and everything that settles there is renamed, sharded and indexed into
    /// the Sorting Box. See `fs::watch`.
    pub fn inbox_dir(&self) -> PathBuf {
        self.root.join("inbox")
    }

    /// Create the app-owned directory tree. Touches nothing else in the root.
    pub fn ensure_dirs(&self) -> Result<()> {
        for dir in [
            self.ggallery_dir(),
            self.cache_dir(),
            self.thumbs_dir(),
            self.sprites_dir(),
            self.trash_dir(),
            self.pending_dir(),
            self.backups_dir(),
            self.files_dir(),
            self.inbox_dir(),
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

    /// Absolute path of the file backing an item — a pure function of its own
    /// uuid (PLAN.md decision 30). Delegates the actual sharding to
    /// `fs::shard`, the one module that owns it.
    pub fn item_path(&self, uuid: &str, ext: &str) -> PathBuf {
        self.files_dir().join(crate::fs::shard::item_rel(uuid, ext))
    }

    /// Where a trashed item's file lives — same sharding as `item_path`, per
    /// PLAN.md decision 30: "the old 'relative path preserved' no longer
    /// describes anything."
    pub fn trash_item_path(&self, uuid: &str, ext: &str) -> PathBuf {
        self.trash_dir().join(crate::fs::shard::item_rel(uuid, ext))
    }

    pub fn thumb_path(&self, uuid: &str) -> PathBuf {
        self.thumbs_dir().join(shard(uuid))
    }

    pub fn sprite_path(&self, uuid: &str) -> PathBuf {
        self.sprites_dir().join(shard(uuid))
    }

    /// True for anything under `<root>/.ggallery` — the app's own storage,
    /// never indexed as library content.
    pub fn is_ggallery_dir(&self, abs: &Path) -> bool {
        abs.starts_with(self.ggallery_dir())
    }
}

/// True for a top-level library-root entry the app already owns — its own
/// reserved directories, or anything hidden — shared by every piece of code
/// that either scans the root's own top level or sweeps it into `inbox/`
/// (`fs::import`, `fs::walk`, `fs::watch`), so the reserved set is defined
/// exactly once.
pub fn is_reserved_top_level(name: &str) -> bool {
    name.starts_with('.') || name.eq_ignore_ascii_case("files") || name.eq_ignore_ascii_case("inbox")
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

/// If `name` is already exactly `<uuid>.<ext>` — the shape this app gives
/// every file it renames — return the canonical (lowercase, hyphenated) uuid.
/// `None` for anything else, including a bare 32-character hex string that
/// happens to parse: that is not a name this app ever wrote.
///
/// Used to recognise a file that arrived already carrying its final name
/// because the M1.7 import rename ran before this walk ever saw it — the
/// walker must reuse that embedded uuid as the item's identity rather than
/// minting a new one, or `disk_name` and `uuid` desync forever.
pub fn parse_uuid_disk_name(name: &str) -> Option<String> {
    let (stem, ext) = name.rsplit_once('.')?;
    if ext.is_empty() || stem.len() != 36 {
        return None;
    }
    let uuid = uuid::Uuid::parse_str(stem).ok()?;
    let canonical = uuid.to_string();
    if canonical != stem {
        return None; // not the exact lowercase hyphenated form this app writes
    }
    Some(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    #[test]
    fn migrate_legacy_dir_renames_gallery_to_ggallery() {
        let root = scratch("paths-migrate-legacy");
        std::fs::create_dir_all(root.join(".gallery")).unwrap();
        std::fs::write(root.join(".gallery/library.db"), b"db bytes").unwrap();

        let paths = LibraryPaths::new(&root);
        paths.migrate_legacy_dir().unwrap();

        assert!(!root.join(".gallery").exists());
        assert!(paths.ggallery_dir().is_dir());
        assert!(paths.db_path().is_file());
    }

    #[test]
    fn migrate_legacy_dir_is_a_no_op_with_nothing_or_already_migrated() {
        let root = scratch("paths-migrate-legacy-noop");
        let paths = LibraryPaths::new(&root);
        paths.migrate_legacy_dir().unwrap();
        assert!(!paths.ggallery_dir().exists());

        std::fs::create_dir_all(paths.ggallery_dir()).unwrap();
        paths.migrate_legacy_dir().unwrap();
        assert!(paths.ggallery_dir().is_dir());
    }

    #[test]
    fn migrate_legacy_dir_refuses_to_guess_when_both_exist() {
        let root = scratch("paths-migrate-legacy-both");
        std::fs::create_dir_all(root.join(".gallery")).unwrap();
        std::fs::create_dir_all(root.join(".ggallery")).unwrap();

        let paths = LibraryPaths::new(&root);
        let err = paths.migrate_legacy_dir().unwrap_err();
        assert!(err.to_string().contains(".gallery"));
        // Neither directory is touched — the caller has to resolve this by hand.
        assert!(root.join(".gallery").is_dir());
        assert!(root.join(".ggallery").is_dir());
    }

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

    #[test]
    fn recognises_only_names_this_app_actually_writes() {
        let uuid = "550e8400-e29b-41d4-a716-446655440000";
        assert_eq!(
            parse_uuid_disk_name(&format!("{uuid}.jpg")),
            Some(uuid.to_string())
        );
        assert_eq!(
            parse_uuid_disk_name(&format!("{}.jpg", uuid.to_uppercase())),
            None,
            "case-sensitive — this app only ever writes lowercase"
        );
        assert_eq!(parse_uuid_disk_name("holiday.jpg"), None);
        assert_eq!(
            parse_uuid_disk_name("550e8400e29b41d4a716446655440000.jpg"),
            None,
            "bare hex, no hyphens — not a name this app wrote"
        );
        assert_eq!(parse_uuid_disk_name("noextension"), None);
    }
}
