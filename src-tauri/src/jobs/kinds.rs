//! Job types and their payloads.
//!
//! Priority orders the whole pipeline. Thumbnails outrank hashing so that each
//! item gets its picture as soon as it is known, rather than the grid staying
//! empty until every file in a 300GB library has been read. Sprites come last
//! — they are the most expensive per item and the least missed.

use serde::{Deserialize, Serialize};

pub const INDEX: &str = "index";
pub const HASH: &str = "hash";
pub const THUMB: &str = "thumb";
pub const SPRITE: &str = "sprite";

pub const PRIORITY_INDEX: i64 = 100;
pub const PRIORITY_THUMB: i64 = 20;
pub const PRIORITY_HASH: i64 = 10;
pub const PRIORITY_SPRITE: i64 = 1;

/// How many times a job is attempted before it is parked as failed. Two: one
/// for a transient lock or a file still being written, and no more, because a
/// file the app cannot read will not become readable by trying harder.
pub const MAX_ATTEMPTS: i64 = 2;

/// A file the walker found. The item row does not exist yet — it is created
/// once the file has actually been read, so a row never exists without a hash.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashPayload {
    pub folder_id: i64,
    pub disk_name: String,
}

/// Thumbnail and sprite jobs, which work from an existing item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ItemPayload {
    pub item_id: i64,
}
