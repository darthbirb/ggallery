//! The persistent job queue's storage. The runner lives in `jobs/`.
//!
//! Completed jobs are deleted rather than kept as `status = 'done'` rows: a
//! full index enqueues three jobs per item, and 300k tombstones would sit in
//! front of the queue index forever. Failures are kept, with their error, so
//! they can be inspected and retried.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::Serialize;

use crate::db::now;
use crate::error::Result;

#[derive(Debug, Clone)]
pub struct QueuedJob {
    pub id: i64,
    pub kind: String,
    pub payload: String,
    pub attempts: i64,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Counts {
    pub pending: i64,
    pub running: i64,
    pub failed: i64,
}

pub fn enqueue(conn: &Connection, kind: &str, payload: &str, priority: i64) -> Result<i64> {
    conn.execute(
        "INSERT INTO job (type, payload, status, priority, created_at)
         VALUES (?1, ?2, 'pending', ?3, ?4)",
        params![kind, payload, priority, now()],
    )?;
    Ok(conn.last_insert_rowid())
}

/// True if a job of this kind and payload is already queued or running — the
/// guard against a second index run duplicating the whole queue.
pub fn is_queued(conn: &Connection, kind: &str, payload: &str) -> Result<bool> {
    let found: Option<i64> = conn
        .query_row(
            "SELECT id FROM job
              WHERE type = ?1 AND payload = ?2 AND status IN ('pending', 'running')
              LIMIT 1",
            params![kind, payload],
            |r| r.get(0),
        )
        .optional()?;
    Ok(found.is_some())
}

/// Take the highest-priority pending job. An immediate transaction serialises
/// workers against each other; `busy_timeout` covers the wait.
pub fn claim(conn: &mut Connection) -> Result<Option<QueuedJob>> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = tx
        .query_row(
            "SELECT id, type, payload, attempts FROM job
              WHERE status = 'pending'
              ORDER BY priority DESC, id
              LIMIT 1",
            [],
            |r| {
                Ok(QueuedJob {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    payload: r.get(2)?,
                    attempts: r.get(3)?,
                })
            },
        )
        .optional()?;

    if let Some(job) = &job {
        tx.execute(
            "UPDATE job SET status = 'running', attempts = attempts + 1 WHERE id = ?1",
            params![job.id],
        )?;
    }
    tx.commit()?;
    Ok(job)
}

pub fn complete(conn: &Connection, id: i64) -> Result<()> {
    conn.execute("DELETE FROM job WHERE id = ?1", params![id])?;
    Ok(())
}

/// Back to `pending` while attempts remain, otherwise `failed` with the error
/// kept for inspection.
pub fn fail(conn: &Connection, id: i64, error: &str, retry: bool) -> Result<()> {
    let status = if retry { "pending" } else { "failed" };
    conn.execute(
        "UPDATE job SET status = ?1, error = ?2 WHERE id = ?3",
        params![status, error, id],
    )?;
    Ok(())
}

pub fn counts(conn: &Connection) -> Result<Counts> {
    let mut counts = Counts::default();
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM job GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for row in rows {
        let (status, n) = row?;
        match status.as_str() {
            "pending" => counts.pending = n,
            "running" => counts.running = n,
            "failed" => counts.failed = n,
            _ => {}
        }
    }
    Ok(counts)
}

/// Jobs left `running` by a crash belong back in the queue at startup.
/// One failure, named well enough to act on: which file, doing what, and what
/// the underlying library actually said.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Failure {
    pub job_id: i64,
    /// `hash`, `thumb`, `sprite` — what was being attempted.
    pub stage: String,
    /// Library-relative folder, empty string at the root.
    pub folder: String,
    pub name: String,
    pub error: String,
    pub attempts: i64,
    pub size_bytes: Option<i64>,
}

/// Every failed job, resolved to the file it was about.
///
/// A thumbnail or sprite job carries an item id; a hash job carries a folder
/// and a filename, because its item row does not exist yet. Both are resolved
/// here so the caller gets one shape.
pub fn failures(conn: &Connection) -> Result<Vec<Failure>> {
    let mut stmt = conn.prepare(
        "SELECT j.id, j.type, j.payload, j.attempts, COALESCE(j.error, 'unknown error')
           FROM job j
          WHERE j.status = 'failed'
          ORDER BY j.id",
    )?;

    let rows: Vec<(i64, String, String, i64, String)> = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<rusqlite::Result<_>>()?;

    let mut out = Vec::with_capacity(rows.len());
    for (job_id, stage, payload, attempts, error) in rows {
        let value: serde_json::Value = serde_json::from_str(&payload).unwrap_or_default();

        let (folder, name, size_bytes) = if let Some(item_id) =
            value.get("itemId").or_else(|| value.get("item_id")).and_then(|v| v.as_i64())
        {
            conn.query_row(
                "SELECT f.rel_path, COALESCE(i.orig_name, i.disk_name), i.size_bytes
                   FROM item i JOIN folder f ON f.id = i.folder_id
                  WHERE i.id = ?1",
                params![item_id],
                |r| Ok((r.get(0)?, r.get(1)?, Some(r.get(2)?))),
            )
            .optional()?
            .unwrap_or_else(|| (String::new(), format!("item {item_id}"), None))
        } else {
            let disk_name = value
                .get("diskName")
                .or_else(|| value.get("disk_name"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown file")
                .to_string();
            let folder = value
                .get("folderId")
                .or_else(|| value.get("folder_id"))
                .and_then(|v| v.as_i64())
                .and_then(|id| crate::db::folders::rel_for(conn, id).ok().flatten())
                .unwrap_or_default();
            (folder, disk_name, None)
        };

        out.push(Failure {
            job_id,
            stage,
            folder,
            name,
            error,
            attempts,
            size_bytes,
        });
    }
    Ok(out)
}

/// Drop every failed job. Called when an index run starts: those failures
/// describe the previous run, the files are about to be attempted again, and
/// leaving them behind is what made "12 failed" mean "four files, three runs
/// ago" instead of something the user could act on.
pub fn clear_failed(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("DELETE FROM job WHERE status = 'failed'", [])?)
}

pub fn requeue_running(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE job SET status = 'pending' WHERE status = 'running'",
        [],
    )?)
}

pub fn retry_failed(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE job SET status = 'pending', attempts = 0, error = NULL WHERE status = 'failed'",
        [],
    )?)
}

pub fn last_error(conn: &Connection) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT error FROM job WHERE status = 'failed' AND error IS NOT NULL
              ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}
