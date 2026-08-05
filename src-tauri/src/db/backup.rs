//! Rolling copies of `library.db`. Since PLAN.md decision 30 the database is
//! the only *structured* copy of the organisation — `library.jsonl` is the
//! plaintext rebuild path, this is the binary one — and it is small next to
//! the media it describes, which is what makes keeping several copies cheap
//! rather than a real cost to weigh.

use std::path::PathBuf;

use crate::error::Result;
use crate::fs::paths::LibraryPaths;

/// How many rolling backups to keep. Small and fixed — a handful of recent
/// snapshots, not a full history.
const KEEP: usize = 5;

/// Copy the (already checkpointed) `library.db` into `.gallery/backups/`,
/// timestamped, then prune down to the `KEEP` most recent. Called from
/// `Library::close`, after `db::checkpoint` has collapsed the WAL — a
/// mid-WAL copy would not be a single, restorable file.
pub fn rotate(paths: &LibraryPaths) -> Result<()> {
    std::fs::create_dir_all(paths.backups_dir())?;
    let dest = paths.backups_dir().join(format!("library-{}.db", crate::db::now()));
    std::fs::copy(paths.db_path(), &dest)?;
    prune(paths)
}

fn prune(paths: &LibraryPaths) -> Result<()> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(paths.backups_dir())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|ext| ext == "db").unwrap_or(false))
        .collect();
    // Filenames are `library-<unix-seconds>.db` — lexical order is
    // chronological order for any timestamp that hasn't rolled over to more
    // digits, which is not a concern for unix seconds until the year 2286.
    backups.sort();
    while backups.len() > KEEP {
        let oldest = backups.remove(0);
        let _ = std::fs::remove_file(oldest);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-libraries")
            .join(name);
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create scratch library");
        root
    }

    fn open_db(root: &std::path::Path) -> LibraryPaths {
        let paths = LibraryPaths::new(root);
        paths.ensure_dirs().unwrap();
        let mut conn = crate::db::open(&paths.db_path()).unwrap();
        crate::db::migrate(&mut conn).unwrap();
        crate::db::checkpoint(&conn).unwrap();
        paths
    }

    #[test]
    fn rotate_copies_the_database_into_backups() {
        let root = scratch("backup-rotate");
        let paths = open_db(&root);

        rotate(&paths).unwrap();

        let backups: Vec<_> = std::fs::read_dir(paths.backups_dir()).unwrap().flatten().collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn rotate_prunes_down_to_the_keep_window() {
        let root = scratch("backup-prune");
        let paths = open_db(&root);

        for i in 0..(KEEP + 3) {
            // Distinct filenames without needing real elapsed time between
            // calls — `rotate` itself derives the name from `now()`, so this
            // pre-seeds extra ones directly to exercise pruning in one shot.
            std::fs::write(paths.backups_dir().join(format!("library-{i}.db")), b"x").unwrap();
        }

        prune(&paths).unwrap();

        let backups: Vec<_> = std::fs::read_dir(paths.backups_dir()).unwrap().flatten().collect();
        assert_eq!(backups.len(), KEEP);
    }

    #[test]
    fn pruning_keeps_the_most_recent_by_name() {
        let root = scratch("backup-prune-order");
        let paths = open_db(&root);

        for i in 0..(KEEP + 2) {
            std::fs::write(paths.backups_dir().join(format!("library-{i:010}.db")), b"x").unwrap();
        }
        prune(&paths).unwrap();

        assert!(!paths.backups_dir().join(format!("library-{:010}.db", 0)).exists(), "oldest pruned");
        assert!(paths.backups_dir().join(format!("library-{:010}.db", KEEP + 1)).exists(), "newest kept");
    }
}
