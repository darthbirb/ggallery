# Data model

SQLite, WAL mode, checkpointed to a single file on clean exit. Lives at
`<root>/.gallery/library.db`.

**All paths are relative to root, forward slashes, normalised case.** No absolute path
ever enters the database.

---

## Schema

### Folders

```sql
CREATE TABLE folder (
  id            INTEGER PRIMARY KEY,
  rel_path      TEXT    NOT NULL UNIQUE,   -- 'People/ana'
  title         TEXT    NOT NULL,          -- 'Ana'  (display, ≠ dirname)
  parent_id     INTEGER REFERENCES folder(id) ON DELETE CASCADE,
  archetype_id  INTEGER REFERENCES archetype(id),
  cover_item_id INTEGER REFERENCES item(id),
  status        TEXT    NOT NULL DEFAULT 'active',  -- active|wip|done|archived
  favorite      INTEGER NOT NULL DEFAULT 0,         -- pinned in sidebar
  notes         TEXT,
  last_added_at INTEGER,                   -- newest item added beneath, recursive
  created_at    INTEGER NOT NULL
);
CREATE INDEX idx_folder_parent ON folder(parent_id);
CREATE INDEX idx_folder_status ON folder(status, last_added_at);

CREATE TABLE folder_status (               -- user-editable value set
  key     TEXT PRIMARY KEY,                -- 'wip'
  label   TEXT NOT NULL,                   -- 'WIP'
  colour  TEXT NOT NULL,
  ordinal INTEGER NOT NULL
);
```

`last_added_at` is maintained recursively — adding an item bubbles the timestamp up
through every ancestor. That is what makes `status:wip sort:staleness` a usable to-do
list rather than a set of labels you forgot you applied.

`title` is what the user typed; the last segment of `rel_path` is derived from it by
sanitising for the filesystem, so a title may legitimately differ from its directory name
(forbidden characters, length caps, sibling collisions). They are not independently
editable — renaming a folder changes both. See [DESIGN.md](DESIGN.md) §1 *Folder names*.

### Items

```sql
CREATE TABLE item (
  id           INTEGER PRIMARY KEY,
  uuid         TEXT    NOT NULL UNIQUE,     -- identity, and the cache key
  folder_id    INTEGER NOT NULL REFERENCES folder(id),
  disk_name    TEXT    NOT NULL,            -- actual filename on disk
  ext          TEXT    NOT NULL,
  orig_name    TEXT,                        -- searchable, pre-import name
  hash         TEXT    NOT NULL,            -- blake3 of content
  size_bytes   INTEGER NOT NULL,
  mtime        INTEGER NOT NULL,
  kind         TEXT    NOT NULL,            -- image | video | other
  width        INTEGER,
  height       INTEGER,
  duration_ms  INTEGER,
  codec        TEXT,
  bitrate      INTEGER,
  captured_at  INTEGER,                     -- EXIF → container → mtime → override
  captured_src TEXT,                        -- exif|container|mtime|manual
  added_at     INTEGER NOT NULL,
  favorite     INTEGER NOT NULL DEFAULT 0,
  notes        TEXT,
  phash        BLOB,
  deleted_at   INTEGER,                     -- soft delete → .gallery/trash
  derived_from INTEGER REFERENCES item(id), -- compression lineage
  download_id  INTEGER REFERENCES download(id)
);
CREATE UNIQUE INDEX idx_item_disk ON item(folder_id, disk_name COLLATE NOCASE);
CREATE INDEX idx_item_folder   ON item(folder_id) WHERE deleted_at IS NULL;
CREATE INDEX idx_item_hash     ON item(hash);
CREATE INDEX idx_item_captured ON item(captured_at);
CREATE INDEX idx_item_phash    ON item(phash);
CREATE INDEX idx_item_favorite ON item(favorite) WHERE favorite = 1;
```

`favorite` is a column rather than a tag on purpose — it needs a single keystroke, a
badge, and its own sidebar entry, and it should never appear in the tag vocabulary
alongside `beach` and `blurry`. `captured_src` keeps a manual override visibly distinct
from real metadata.

The file on disk is `<folder.rel_path>/<disk_name>`. Path is derived, never stored on
the item — so moving a folder is a single row update, not a rewrite of every child.

`disk_name` exists because M1 is strictly read-only over the library: files keep whatever
names they already have, so the app has to remember them. After M1.5 renames everything,
`disk_name` is exactly `<uuid>.<ext>` and the two agree. `uuid` is the item's identity
from the moment it is first indexed and is what the thumbnail and sprite caches are keyed
by, so it is issued at index time rather than at rename time — otherwise the rename would
orphan every cached thumbnail.

### Tags

One table covers both shapes. `key IS NULL` means a flag; otherwise it is a label.

```sql
CREATE TABLE tag (
  id    INTEGER PRIMARY KEY,
  key   TEXT,               -- 'instagram', or NULL for a flag
  value TEXT NOT NULL,      -- '@ana' / 'beach'
  UNIQUE(key, value)
);
CREATE INDEX idx_tag_value ON tag(value COLLATE NOCASE);
CREATE INDEX idx_tag_key   ON tag(key   COLLATE NOCASE);
```

A label with no value yet is stored with `value = ''` so archetype fields exist and
render before they are filled.

```sql
-- tags attached directly to a folder
CREATE TABLE folder_tag (
  folder_id INTEGER NOT NULL REFERENCES folder(id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tag(id),
  source    TEXT    NOT NULL,   -- title | archetype | manual
  PRIMARY KEY (folder_id, tag_id)
);

-- tags attached directly to an item
CREATE TABLE item_tag (
  item_id  INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  tag_id   INTEGER NOT NULL REFERENCES tag(id),
  added_at INTEGER NOT NULL,
  PRIMARY KEY (item_id, tag_id)
);
CREATE INDEX idx_item_tag_rev ON item_tag(tag_id, item_id);

-- materialised union of inherited + manual, rebuilt on invalidation
CREATE TABLE item_effective_tag (
  item_id   INTEGER NOT NULL REFERENCES item(id) ON DELETE CASCADE,
  tag_id    INTEGER NOT NULL REFERENCES tag(id),
  origin_id INTEGER REFERENCES folder(id),  -- NULL when manual
  PRIMARY KEY (item_id, tag_id)
);
CREATE INDEX idx_eff_rev ON item_effective_tag(tag_id, item_id);

-- alternate spellings resolving to one canonical tag
CREATE TABLE tag_alias (
  alias  TEXT NOT NULL,
  tag_id INTEGER NOT NULL REFERENCES tag(id) ON DELETE CASCADE,
  PRIMARY KEY (alias COLLATE NOCASE)
);
```

`source = 'title'` rows are managed by the app and cannot be removed by the user — that
is how "the folder name is always a base tag" is enforced.

### Archetypes

```sql
CREATE TABLE archetype (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE          -- 'Person'
);

CREATE TABLE archetype_field (
  id           INTEGER PRIMARY KEY,
  archetype_id INTEGER NOT NULL REFERENCES archetype(id) ON DELETE CASCADE,
  key          TEXT    NOT NULL,     -- 'instagram'
  ordinal      INTEGER NOT NULL,
  UNIQUE(archetype_id, key)
);
```

**A field is a name and a position.** It carried a `type` — text, handle, url, date,
number — until M2.5a.1 dropped the column. The only behaviour any of them implied was
`handle` rendering as a link to a platform profile, which locked decision 21 removed;
after that the editor was asking a question the app ignored. Values are text, and every
field renders the same way.

### Queues and history

```sql
CREATE TABLE download (
  id           INTEGER PRIMARY KEY,
  url          TEXT NOT NULL UNIQUE,
  tool         TEXT NOT NULL,        -- yt-dlp | gallery-dl
  site         TEXT,
  uploader     TEXT,
  status       TEXT NOT NULL,
  dest_rel_dir TEXT,
  raw_meta     TEXT,                 -- JSON from the tool
  created_at   INTEGER NOT NULL
);

CREATE TABLE compression (
  id             INTEGER PRIMARY KEY,
  item_id        INTEGER NOT NULL REFERENCES item(id),
  preset         TEXT    NOT NULL,
  pending_path   TEXT,               -- under .gallery/pending
  orig_size      INTEGER NOT NULL,
  new_size       INTEGER,
  status         TEXT    NOT NULL,   -- running | pending_review | kept | rejected
  created_at     INTEGER NOT NULL
);
CREATE INDEX idx_compression_status ON compression(status);

CREATE TABLE job (
  id         INTEGER PRIMARY KEY,
  type       TEXT NOT NULL,   -- index|hash|thumb|sprite|phash|transcode|download|move
  payload    TEXT NOT NULL,   -- JSON
  status     TEXT NOT NULL,   -- pending|running|done|failed
  priority   INTEGER NOT NULL DEFAULT 0,
  attempts   INTEGER NOT NULL DEFAULT 0,
  error      TEXT,
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_job_queue ON job(status, priority DESC, id);
-- Completed jobs are deleted rather than left as `done` rows: a full index
-- queues three jobs per item, and 300k tombstones would sit in front of this
-- index forever. Failures are kept, with their error, so they can be retried.

CREATE TABLE journal (         -- undo stack, survives restarts
  id         INTEGER PRIMARY KEY,
  op         TEXT NOT NULL,    -- move | tag | untag | delete | rename | compress
  forward    TEXT NOT NULL,    -- JSON
  inverse    TEXT NOT NULL,    -- JSON
  batch_id   TEXT NOT NULL,    -- one Ctrl+Z undoes a whole batch
  created_at INTEGER NOT NULL
);
CREATE INDEX idx_journal_batch ON journal(batch_id);

CREATE TABLE dupe_pair (
  a_id INTEGER NOT NULL REFERENCES item(id),
  b_id INTEGER NOT NULL REFERENCES item(id),
  distance INTEGER NOT NULL,
  resolution TEXT,             -- NULL | kept_a | kept_b | dismissed
  PRIMARY KEY (a_id, b_id)
);

CREATE TABLE saved_search (id INTEGER PRIMARY KEY, name TEXT NOT NULL, query TEXT NOT NULL, pinned INTEGER);
CREATE TABLE setting       (key TEXT PRIMARY KEY, value TEXT NOT NULL);
CREATE TABLE schema_version(version INTEGER NOT NULL);

CREATE VIRTUAL TABLE item_fts USING fts5(orig_name, tags, content='');
```

---

## Tag resolution

<a id="tag-resolution"></a>

An item's effective tags are the union of every ancestor folder's tags plus its own:

```sql
WITH RECURSIVE ancestry(id) AS (
    SELECT folder_id FROM item WHERE id = :item
  UNION ALL
    SELECT f.parent_id FROM folder f JOIN ancestry a ON f.id = a.id
    WHERE f.parent_id IS NOT NULL
)
SELECT tag_id, folder_id AS origin_id FROM folder_tag
  WHERE folder_id IN (SELECT id FROM ancestry)
UNION
SELECT tag_id, NULL FROM item_tag WHERE item_id = :item;
```

That recursion is too slow to run per query at 100k items, so results are materialised
into `item_effective_tag`. **Rebuild is triggered by:**

| Event | Scope of rebuild |
| --- | --- |
| Item moves between folders | that item |
| Item's manual tags change | that item |
| Folder's tags change | all items recursively beneath it |
| Folder moves or is renamed | all items recursively beneath it |
| Archetype applied to a folder | all items recursively beneath it |
| Folder created or deleted | its subtree |

Rebuilds run as jobs so a folder-level change never blocks the UI. At ~10 tags per item
the table lands around 1M rows at full scale, which SQLite handles without complaint.

**Live inheritance means a move recomputes, it does not accumulate.** When a move would
drop inherited tags, the UI offers to convert specific ones into `item_tag` rows before
committing — see [DESIGN.md](DESIGN.md#core-concepts).

---

## Query language

<a id="query-language"></a>

A recursive-descent parser compiling to SQL. Terms are ANDed implicitly.

```
path:People/Ana        prefix match, recursive
path:=People/Ana       that folder only

tag:beach              flag, exact
tag:bea*               flag, prefix
instagram:@ana         label by key and value
instagram:*            label present, any value (including empty)
:@ana                  any label whose value matches
@ana                   bare — matches values across all keys

type:video             image | video
year:2024
date:2024-06..2024-08
dur:>30s               s | m | h
size:>100mb            kb | mb | gb
w:>=1920  h:>=1080

source:instagram
uploader:@ana

is:favorite
is:untagged            no manual tags
is:unsorted            in the Sorting Box
is:duplicate           has an unresolved dupe_pair
is:compressed          derived_from IS NOT NULL
is:pending             awaiting compression review
is:trashed

status:wip             folder status — active | wip | done | archived
                       matches folders directly, and items beneath them

-tag:blurry            negation
(a or b) c             explicit OR, grouping
sunset                 bare word with no operator → FTS over orig_name and tags
```

**Resolution order for a bare term** — try label value, then flag value, then folder
title, then fall through to FTS. The search dropdown shows which interpretation matched
so an ambiguous term is never silently wrong.

Sidebar interactions mutate this string rather than bypassing it: clicking a folder
appends `path:`, ctrl-clicking a tag appends `tag:`, alt-clicking prepends `-`. The bar
stays directly editable.

---

## Notes for implementation

- **WAL checkpoint on exit** (`PRAGMA wal_checkpoint(TRUNCATE)`) so a closed library is
  a single `.db` file, safe to copy.
- **Content hash is identity across renames and moves.** Reconciliation after external
  tools touch the folder is a hash lookup, not a guess.
- **Compression breaks the hash**, which is what `derived_from` exists for. Accepting a
  compressed file keeps the same `item.id`, assigns a new `uuid`, and points
  `derived_from` at the trashed original — so tags, folder placement and history all
  survive untouched.
- **`library.jsonl`** is written on a debounce: one line per item with uuid, folder path,
  orig_name, hash, and resolved tags. It is the rebuild path if the database is ever
  lost, and the disaster-recovery record of what the first-import rename did — no
  tooling reads it back to reconstruct names; see docs/DESIGN.md#first-import.
