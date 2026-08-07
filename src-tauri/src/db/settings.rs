//! The generic `setting` key/value table.
//!
//! First real use: the `imported_at` marker that decides whether the
//! first-import wizard is still owed. See SPEC.md#first-import.

use rusqlite::{params, Connection, OptionalExtension};

use crate::db::now;
use crate::error::Result;

const IMPORTED_AT: &str = "imported_at";
const STORAGE_MIGRATION_VERIFIED_AT: &str = "storage_migration_verified_at";

/// `None` until the wizard (or the "Normalise filenames" repair action) has
/// run to completion at least once. See `fs::import::execute`, which sets
/// this, and `mark_imported`, which the frontend calls directly for a
/// library that never needed the ceremony in the first place.
pub fn imported_at(conn: &Connection) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT value FROM setting WHERE key = ?1",
            params![IMPORTED_AT],
            |r| r.get::<_, String>(0),
        )
        .optional()?
        .and_then(|v| v.parse().ok()))
}

/// Idempotent — safe to call on a library that is already marked imported.
pub fn mark_imported(conn: &Connection) -> Result<()> {
    conn.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![IMPORTED_AT, now().to_string()],
    )?;
    Ok(())
}

/// `None` until `fs::shard`'s physical storage migration has moved every
/// file and `verify` has confirmed it — set from the *pre-schema-migration*
/// (v7) connection the wizard commands open directly, since `Library::open`
/// refuses to apply migration 008 until this is set. See
/// `db::needs_storage_migration` and ROADMAP.md §M2.6.
pub fn storage_migration_verified_at(conn: &Connection) -> Result<Option<i64>> {
    Ok(get(conn, STORAGE_MIGRATION_VERIFIED_AT)?.and_then(|v| v.parse().ok()))
}

pub fn mark_storage_migration_verified(conn: &Connection) -> Result<()> {
    set(conn, STORAGE_MIGRATION_VERIFIED_AT, &now().to_string())
}

/// The generic get/set pair `imported_at`/`mark_imported` could have been
/// written against, for anything that just needs a marker or a small stored
/// value — `fs::lowercase_migration`'s "has this library already been
/// folded" gate, for one.
pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row("SELECT value FROM setting WHERE key = ?1", params![key], |r| r.get(0))
        .optional()?)
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO setting (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn memory_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        db::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn unset_until_marked_then_idempotent() {
        let conn = memory_conn();
        assert!(imported_at(&conn).unwrap().is_none());

        mark_imported(&conn).unwrap();
        let first = imported_at(&conn).unwrap().expect("now set");

        mark_imported(&conn).unwrap();
        let second = imported_at(&conn).unwrap().expect("still set");
        assert!(second >= first, "a second mark only ever moves it forward");
    }
}
