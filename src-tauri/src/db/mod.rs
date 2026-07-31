//! Connection handling and migrations. **All SQL in the application lives
//! under this module** — commands call functions, never queries.

pub mod folders;
pub mod items;
pub mod jobs;

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::Connection;

use crate::error::Result;

/// Numbered migrations, applied in order, never edited once shipped.
const MIGRATIONS: &[(i64, &str)] = &[(1, include_str!("migrations/001_initial.sql"))];

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

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
