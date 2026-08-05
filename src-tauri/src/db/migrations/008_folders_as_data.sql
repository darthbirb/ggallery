-- PLAN.md decision 30 / §M2.6: folders are data, not directories. This is the
-- schema half only — it must never run until `fs::shard`'s physical
-- migration has moved every file to `files/<xx>/<uuid>.<ext>` and verified
-- clean. `Library::open` gates on that; see `db::needs_storage_migration`.
--
-- Table-rebuild pattern throughout, not `ALTER TABLE ... DROP COLUMN`:
-- `rel_path` carries a `UNIQUE` constraint, and SQLite refuses to drop a
-- column that is part of one.
--
-- `defer_foreign_keys` (not `PRAGMA foreign_keys`, which is a no-op inside a
-- transaction) pushes every FK check to commit time. Without it, dropping
-- `folder` while `folder_tag`/`item_tag`/`item_effective_tag` still carry
-- `REFERENCES folder(id)` — and populating `folder_new`'s self-referential
-- `parent_id` in whatever row order `SELECT` happens to return — would both
-- be checked against a schema that is momentarily inconsistent mid-script.
-- By commit, both tables exist again under their real names with matching
-- ids, so everything resolves.
PRAGMA defer_foreign_keys = ON;

-- The library root was a real row (rel_path = '', parent_id NULL) that stood
-- in for "everything unfiled". Decision 30 stops treating the Sorting Box as
-- a place: its direct children become real top-level folders (parent_id
-- NULL), its direct items become properly unfiled (folder_id NULL), and the
-- row itself goes — not kept as a sentinel.
UPDATE folder
   SET parent_id = NULL
 WHERE parent_id = (SELECT id FROM folder WHERE rel_path = '');

UPDATE item
   SET folder_id = NULL
 WHERE folder_id = (SELECT id FROM folder WHERE rel_path = '');

DELETE FROM folder WHERE rel_path = '';

CREATE TABLE folder_new (
  id            INTEGER PRIMARY KEY,
  title         TEXT    NOT NULL,
  parent_id     INTEGER REFERENCES folder_new(id) ON DELETE CASCADE,
  archetype_id  INTEGER REFERENCES archetype(id),
  cover_item_id INTEGER REFERENCES item(id),
  status        TEXT    NOT NULL DEFAULT 'active',
  favorite      INTEGER NOT NULL DEFAULT 0,
  notes         TEXT,
  last_added_at INTEGER,
  created_at    INTEGER NOT NULL,
  deleted_at    INTEGER
);
INSERT INTO folder_new (id, title, parent_id, archetype_id, cover_item_id, status, favorite, notes, last_added_at, created_at, deleted_at)
  SELECT id, title, parent_id, archetype_id, cover_item_id, status, favorite, notes, last_added_at, created_at, deleted_at
    FROM folder;
DROP TABLE folder;
ALTER TABLE folder_new RENAME TO folder;

CREATE INDEX idx_folder_parent ON folder(parent_id);
CREATE INDEX idx_folder_status ON folder(status, last_added_at);
-- Partial: a trashed folder's (parent_id, title) must never block a new
-- folder at the same spot, and — since trash no longer has a path to free by
-- rewriting it — this is now the only thing that makes the slot reusable.
--
-- This statement is also the collision check decision 31's lowercase
-- fold-and-merge (007) should already have made unnecessary: two live
-- directories differing only by case could never coexist on Windows, so no
-- live sibling pair should be able to collide on `(parent_id, title)` here.
-- If one somehow does, this fails the whole migration loudly with SQLite's
-- own "UNIQUE constraint failed" rather than silently merging — a merge here
-- would be exactly the "one user's data shape promoted into product
-- behaviour" decision 21 warns against.
CREATE UNIQUE INDEX idx_folder_sibling ON folder(parent_id, title) WHERE deleted_at IS NULL;

CREATE TABLE item_new (
  id           INTEGER PRIMARY KEY,
  uuid         TEXT    NOT NULL UNIQUE,
  folder_id    INTEGER REFERENCES folder(id),
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
INSERT INTO item_new SELECT * FROM item;
DROP TABLE item;
ALTER TABLE item_new RENAME TO item;

-- Globally unique now, not per-folder — a file's name is its shard location,
-- which no folder can any longer disambiguate.
CREATE UNIQUE INDEX idx_item_disk     ON item(disk_name COLLATE NOCASE);
CREATE INDEX        idx_item_folder   ON item(folder_id) WHERE deleted_at IS NULL;
CREATE INDEX        idx_item_hash     ON item(hash);
CREATE INDEX        idx_item_captured ON item(captured_at);
CREATE INDEX        idx_item_phash    ON item(phash);
CREATE INDEX        idx_item_favorite ON item(favorite) WHERE favorite = 1;
