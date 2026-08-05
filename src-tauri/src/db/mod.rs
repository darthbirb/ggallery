//! Connection handling and migrations. **All SQL in the application lives
//! under this module** — commands call functions, never queries.

pub mod backup;
pub mod folders;
pub mod items;
pub mod jobs;
pub mod journal;
pub mod settings;
pub mod tags;

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::Result;

/// Numbered migrations, applied in order, never edited once shipped.
const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("migrations/001_initial.sql")),
    (2, include_str!("migrations/002_folder_metadata.sql")),
    (3, include_str!("migrations/003_drop_seeded_archetypes.sql")),
    (4, include_str!("migrations/004_folder_soft_delete.sql")),
    (5, include_str!("migrations/005_drop_archetype_field_type.sql")),
    (6, include_str!("migrations/006_drop_root_title_tag.sql")),
    (7, include_str!("migrations/007_lowercase_vocabulary.sql")),
    (8, include_str!("migrations/008_folders_as_data.sql")),
];

/// Open a connection with the pragmas the whole app assumes. Every thread that
/// touches the database opens its own; WAL makes that safe for one writer and
/// any number of readers, and `busy_timeout` absorbs the contention between
/// job workers.
pub fn open(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(20))?;
    // journal_mode returns the resulting mode, so it has to be queried.
    let _: String = conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    conn.pragma_update(None, "cache_size", -32_000i64)?;
    Ok(conn)
}

/// The schema version at which `folder.rel_path` (and the directory it named)
/// stop existing — `fs::shard`'s physical migration must have already moved
/// every file and verified clean before this version is ever applied. See
/// `needs_storage_migration`.
pub const STORAGE_MIGRATION_SCHEMA_VERSION: i64 = 8;

/// `0` for a database that predates the `schema_version` table entirely
/// (never migrated at all), otherwise the highest version applied so far.
/// Reads without applying anything — callers that need to *act* on the
/// version before deciding whether `migrate` is even safe to call (this
/// milestone's storage-migration gate) need that separation.
pub fn current_schema_version(conn: &Connection) -> Result<i64> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_version')",
        [],
        |r| r.get(0),
    )?;
    if !exists {
        return Ok(0);
    }
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?)
}

/// True when this library has real pre-M2.6 content (`schema_version`
/// between 1 and 7) that `fs::shard`'s physical migration has not yet moved
/// and verified. A brand-new database (`version == 0`) needs nothing — there
/// is nothing to move — and neither does one already past the storage
/// migration's own version.
pub fn needs_storage_migration(conn: &Connection) -> Result<bool> {
    let version = current_schema_version(conn)?;
    if version == 0 || version >= STORAGE_MIGRATION_SCHEMA_VERSION {
        return Ok(false);
    }
    Ok(settings::storage_migration_verified_at(conn)?.is_none())
}

pub fn migrate(conn: &mut Connection) -> Result<()> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS schema_version (version INTEGER NOT NULL);")?;
    let current: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |r| r.get(0),
    )?;

    for (version, sql) in MIGRATIONS {
        if *version <= current {
            continue;
        }
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.execute(
            "INSERT INTO schema_version (version) VALUES (?1)",
            [version],
        )?;
        tx.commit()?;
    }
    Ok(())
}

/// Collapse the WAL back into the single `.db` file. A closed library must be
/// one file, safe to copy.
pub fn checkpoint(conn: &Connection) -> Result<()> {
    conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    Ok(())
}

// The indexer commits in batches of a few hundred files. Explicit statements
// rather than `Connection::transaction`, because the walk holds its connection
// across the whole run and re-borrowing it per batch fights the borrow checker
// for no benefit.

pub fn begin_batch(conn: &Connection) -> Result<()> {
    conn.execute_batch("BEGIN")?;
    Ok(())
}

pub fn commit_batch(conn: &Connection) -> Result<()> {
    conn.execute_batch("COMMIT")?;
    Ok(())
}

pub fn rollback_batch(conn: &Connection) {
    let _ = conn.execute_batch("ROLLBACK");
}

/// Case-folded on the way in — PLAN.md decision 31. One implementation,
/// called from every place a folder title, a tag key, a tag value or a flag
/// is written (`db::folders::{upsert,create_record,set_title_unjournalled}`,
/// `db::tags::{get_or_create_tag,rename_tag}`), so what ends up stored is
/// exactly what search and display both compare against. Notes, original
/// filenames and every other free-text field never call this.
pub fn fold(text: &str) -> String {
    text.to_lowercase()
}

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
