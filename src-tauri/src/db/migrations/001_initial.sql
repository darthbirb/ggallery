-- Initial schema. Mirrors docs/DATA-MODEL.md.
--
-- Migrations are never edited once shipped. Later milestones add numbered
-- files; they do not touch this one.
--
-- One deliberate addition to the documented `item` table: `disk_name`, the
-- actual filename on disk. DATA-MODEL derives the path as
-- `<folder.rel_path>/<uuid>.<ext>`, which is true only after M1.5 renames
-- everything to UUIDs. M1 is strictly read-only over the library, so files
-- keep the names they already have and the app has to remember them. After
-- M1.5, `disk_name` is `<uuid>.<ext>` and the two agree again.

CREATE TABLE folder (
  id            INTEGER PRIMARY KEY,
  rel_path      TEXT    NOT NULL UNIQUE,
  title         TEXT    NOT NULL,
  parent_id     INTEGER REFERENCES folder(id) ON DELETE CASCADE,
  archetype_id  INTEGER REFERENCES archetype(id),
  cover_item_id INTEGER REFERENCES item(id),
  status        TEXT    NOT NULL DEFAULT 'active',
  favorite      INTEGER NOT NULL DEFAULT 0,
  notes         TEXT,
  last_added_at INTEGER,
  created_at    INTEGER NOT NULL
);
CREATE INDEX idx_folder_parent ON folder(parent_id);
CREATE INDEX idx_folder_status ON folder(status, last_added_at);

CREATE TABLE folder_status (
  key     TEXT PRIMARY KEY,
  label   TEXT NOT NULL,
  colour  TEXT NOT NULL,
  ordinal INTEGER NOT NULL
);

CREATE TABLE item (
  id           INTEGER PRIMARY KEY,
  uuid         TEXT    NOT NULL UNIQUE,
  folder_id    INTEGER NOT NULL REFERENCES folder(id),
  disk_name    TEXT    NOT NULL,
  ext          TEXT    NOT NULL,
  orig_name    TEXT,
  hash         TEXT    NOT NULL,
  size_bytes   INTEGER NOT NULL,
  mtime        INTEGER NOT NULL,
  kind         TEXT    NOT NULL,
  width        INTEGER,
  height       INTEGER,
  duration_ms  INTEGER,
  codec        TEXT,
  bitrate      INTEGER,
  captured_at  INTEGER,
  captured_src TEXT,
  added_at     INTEGER NOT NULL,
  favorite     INTEGER NOT NULL DEFAULT 0,
  notes        TEXT,
  phash        BLOB,
  deleted_at   INTEGER,
  derived_from INTEGER REFERENCES item(id),
  download_id  INTEGER REFERENCES download(id)
);
CREATE UNIQUE INDEX idx_item_disk     ON item(folder_id, disk_name COLLATE NOCASE);
CREATE INDEX        idx_item_folder   ON item(folder_id) WHERE deleted_at IS NULL;
CREATE INDEX        idx_item_hash     ON item(hash);
CREATE INDEX        idx_item_captured ON item(captured_at);
CREATE INDEX        idx_item_phash    ON item(phash);
CREATE INDEX        idx_item_favorite ON item(favorite) WHERE favorite = 1;

CREATE TABLE tag (
  id    INTEGER PRIMARY KEY,
  key   TEXT,
  value TEXT NOT NULL,
  UNIQUE(key, value)
);
CREATE INDEX idx_tag_value ON tag(value COLLATE NOCASE);
CREATE INDEX idx_tag_key   ON tag(key   COLLATE NOCASE);

CREATE TABLE folder_tag (
  folder_id INTEGER NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tag(id),
  source    TEXT    NOT NULL,
  PRIMARY KEY (folder_id, tag_id)
);

CREATE TABLE item_tag (
  item_id  INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  tag_id   INTEGER NOT NULL REFERENCES tag(id),
  added_at INTEGER NOT NULL,
  PRIMARY KEY (item_id, tag_id)
);
CREATE INDEX idx_item_tag_rev ON item_tag(tag_id, item_id);

CREATE TABLE item_effective_tag (
  item_id   INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tag(id),
  origin_id INTEGER REFERENCES folder(id),
  PRIMARY KEY (item_id, tag_id)
);
CREATE INDEX idx_eff_rev ON item_effective_tag(tag_id, item_id);

CREATE TABLE tag_alias (
  alias  TEXT NOT NULL,
  tag_id INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (alias COLLATE NOCASE)
);

CREATE TABLE archetype (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE
);

CREATE TABLE archetype_field (
  id           INTEGER PRIMARY KEY,
  archetype_id INTEGER NOT NULL REFERENCES archetype(id) ON DELETE CASCADE,
  key          TEXT    NOT NULL,
  type         TEXT    NOT NULL,
  ordinal      INTEGER NOT NULL,
  UNIQUE(archetype_id, key)
);

CREATE TABLE download (
  id           INTEGER PRIMARY KEY,
  url          TEXT NOT NULL UNIQUE,
  tool         TEXT NOT NULL,
  site         TEXT,
  uploader     TEXT,
  status       TEXT NOT NULL,
  dest_rel_dir TEXT,
  raw_meta     TEXT,
  created_at   INTEGER NOT NULL
);

CREATE TABLE compression (
  id           INTEGER PRIMARY KEY,
  item_id      INTEGER NOT NULL REFERENCES item(id),
  preset       TEXT    NOT NULL,
  pending_path TEXT,
  orig_size    INTEGER NOT NULL,
  new_size     INTEGER,
  status       TEXT    NOT NULL,
  created_at   INTEGER NOT NULL
);
CREATE INDEX idx_compression_status ON compression(status);

CREATE TABLE job (
  id         INTEGER PRIMARY KEY,
  type       TEXT NOT NULL,
  payload    TEXT NOT NULL,
  status     TEXT NOT NULL,
  priority   INTEGER NOT NULL DEFAULT 0,
  attempts   INTEGER NOT NULL DEFAULT 0,
  error      TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_job_queue ON job(status, priority DESC, id);

CREATE TABLE journal (
  id         INTEGER PRIMARY KEY,
  op         TEXT NOT NULL,
  forward    TEXT NOT NULL,
  inverse    TEXT NOT NULL,
  batch_id   TEXT NOT NULL,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_journal_batch ON journal(batch_id);

CREATE TABLE dupe_pair (
  a_id       INTEGER NOT NULL REFERENCES item(id),
  b_id       INTEGER NOT NULL REFERENCES item(id),
  distance   INTEGER NOT NULL,
  resolution TEXT,
  PRIMARY KEY (a_id, b_id)
);

CREATE TABLE saved_search (
  id     INTEGER PRIMARY KEY,
  name   TEXT NOT NULL,
  query  TEXT NOT NULL,
  pinned INTEGER
);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

CREATE VIRTUAL TABLE item_fts USING fts5(orig_name, tags, content='');
