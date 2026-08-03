/** Shared types, mirroring the Rust structs they arrive as. */

export interface LibraryInfo {
  /** Absolute path of the library root — the only absolute path in the app. */
  root: string;
  name: string;
  /** Absolute cache roots; item thumbnails are relative to these. */
  thumbsDir: string;
  spritesDir: string;
  itemCount: number;
  folderCount: number;
  /** Path of the ffmpeg in use, or null when videos cannot be thumbnailed. */
  ffmpeg: string | null;
}

export interface LibraryStatus {
  info: LibraryInfo | null;
  /** Root remembered in gallery.config.json, if any. */
  remembered: string | null;
}

export interface FolderNode {
  id: number;
  relPath: string;
  title: string;
  parentId: number | null;
  depth: number;
  directCount: number;
  totalCount: number;
  status: string;
  favorite: boolean;
}

export interface GridItem {
  id: number;
  /** `ab/cd/<uuid>.webp`, resolved against thumbsDir or spritesDir. */
  thumb: string;
  kind: "image" | "video" | "other";
  w: number | null;
  h: number | null;
  durationMs: number | null;
  favorite: boolean;
  /** Captured date where known, file mtime otherwise. Unix seconds. */
  at: number;
  name: string;
}

export type Phase = "idle" | "walking" | "working";

export interface Progress {
  phase: Phase;
  folders: number;
  filesSeen: number;
  items: number;
  pending: number;
  running: number;
  failed: number;
  completed: number;
  lastError: string | null;
  /** True while the current walk is a full reconcile the filesystem watcher
   *  triggered after an overflow or error, rather than the ordinary startup
   *  index — see docs/DESIGN.md §10 "The library is live". */
  rescanning: boolean;
}

/** One file that failed to index, with the error the decoder actually gave. */
export interface IndexFailure {
  jobId: number;
  /** What was being attempted: hash, thumb or sprite. */
  stage: string;
  /** Library-relative folder; empty at the root. */
  folder: string;
  name: string;
  error: string;
  attempts: number;
  sizeBytes: number | null;
}

export interface AppError {
  kind: string;
  message: string;
}

// --- M1.5 import wizard ------------------------------------------------

export interface KindTotal {
  kind: string;
  count: number;
  bytes: number;
}

export interface ScanReport {
  byKind: KindTotal[];
  totalItems: number;
  totalBytes: number;
  folderCount: number;
  /** Files M1 could not read at all. */
  unreadable: number;
  alreadyRenamed: number;
  toRename: number;
  /** `null` until the wizard (or "Normalise filenames") has completed once —
   *  what decides whether the wizard is offered when a library is opened. */
  importedAt: number | null;
}

export interface RenamePreview {
  folder: string;
  oldName: string;
  newName: string;
}

export interface DryRunReport {
  toRename: number;
  sample: RenamePreview[];
}

export interface RenameError {
  itemId: number;
  folder: string;
  name: string;
  error: string;
}

export interface ExecuteReport {
  renamed: number;
  alreadyDone: number;
  errors: RenameError[];
}

export interface ImportProgress {
  done: number;
  total: number;
  errors: number;
}

export interface VerifyItem {
  itemId: number;
  folder: string;
  name: string;
}

export interface VerifyReport {
  sampleChecked: number;
  mismatches: VerifyItem[];
  missing: VerifyItem[];
  countTotal: number;
  countRenamed: number;
}

// --- M1.7 startup flow ---------------------------------------------------
//
// Filesystem-only: these run before a library is ever opened, so the report
// shape below only exists as `RenameFsError`/`FsExecuteReport` — see
// src-tauri/src/fs/import.rs.

/** What `prepareImport` found — the Review screen's whole content. */
export interface ReviewReport {
  /** True when this library needs no ceremony at all — already imported, or
   *  nothing to rename. The caller skips Review and Progress and opens
   *  straight into the gallery. */
  alreadyImported: boolean;
  byKind: KindTotal[];
  totalItems: number;
  totalBytes: number;
  folderCount: number;
  /** Entries the scan could not read at all. */
  unreadable: number;
  alreadyRenamed: number;
  toRename: number;
  /** Five rows, not a full manifest. */
  sample: RenamePreview[];
}

export interface FsRenameError {
  folder: string;
  name: string;
  error: string;
}

export interface FsExecuteReport {
  renamed: number;
  errors: FsRenameError[];
}

// --- M2: folders as entities ---------------------------------------------

export interface ArchetypeFieldValue {
  key: string;
  ordinal: number;
  value: string;
}

export interface FolderFlag {
  tagId: number;
  value: string;
}

export interface FolderDetail {
  id: number;
  relPath: string;
  title: string;
  parentId: number | null;
  status: string;
  favorite: boolean;
  notes: string | null;
  lastAddedAt: number | null;
  directCount: number;
  totalCount: number;
  subfolderCount: number;
  archetypeId: number | null;
  archetypeName: string | null;
  fields: ArchetypeFieldValue[];
  flags: FolderFlag[];
  /** Cache-relative thumbnail of the cover: the item chosen, or the newest
   *  item beneath the folder when nothing has been. */
  coverThumb: string | null;
  /** Set only when the cover was chosen rather than picked automatically. */
  coverItemId: number | null;
}

export interface FolderStatusDef {
  key: string;
  label: string;
  colour: string;
  ordinal: number;
}

/** A name and a position. Fields carried a type — text / handle / url / date /
 *  number — until M2.5a.1; nothing ever read it once decision 21 removed the
 *  platform linking it existed for. */
export interface ArchetypeFieldDef {
  key: string;
  ordinal: number;
}

export interface ArchetypeInfo {
  id: number;
  name: string;
  fields: ArchetypeFieldDef[];
}

export interface EffectiveTag {
  tagId: number;
  key: string | null;
  value: string;
  /** `null` for a manual tag; the contributing ancestor folder otherwise. */
  originId: number | null;
}

// --- M2.1: folder/item operations, archetype and status management -------

/** A folder on this archetype that has actually filled the field in —
 *  named in the confirmation before `removeArchetypeField` deletes it. */
export interface ArchetypeFieldUsage {
  folderId: number;
  relPath: string;
  title: string;
  value: string;
}

export interface TagSummary {
  id: number;
  key: string | null;
  value: string;
  usageCount: number;
}

export interface ItemOpError {
  itemId: number;
  error: string;
}

export interface MoveItemsReport {
  moved: number;
  errors: ItemOpError[];
  /** The journal batch — what the toast's Undo button hands to `undoBatch`. */
  batchId: string;
}

export interface TrashItemsReport {
  trashed: number;
  errors: ItemOpError[];
  batchId: string;
}

// --- M2.5a: the pane, and the undo behind the toast ----------------------

/** One item in full — the pane's Preview mode. Wider than `GridItem` on
 *  purpose: this is fetched one row at a time, not 100k. */
export interface ItemDetail {
  id: number;
  kind: "image" | "video" | "other";
  /** Absolute path to the file, for `assetPath()`. The database never stores
   *  one; the backend resolves it per call. */
  path: string;
  /** `ab/cd/<uuid>.webp`, so the preview can show something while the
   *  original decodes. */
  thumb: string;
  diskName: string;
  origName: string | null;
  folderId: number;
  folderRel: string;
  folderTitle: string;
  sizeBytes: number;
  width: number | null;
  height: number | null;
  durationMs: number | null;
  codec: string | null;
  bitrate: number | null;
  capturedAt: number | null;
  /** Where `capturedAt` came from, so a guess is never mistaken for
   *  metadata — DESIGN.md §1 "Items". */
  capturedSrc: string | null;
  addedAt: number;
  favorite: boolean;
  notes: string | null;
  hash: string;
  /** M5 fills this in; null until downloads exist. */
  sourceUrl: string | null;
}

export interface UndoReport {
  reversed: number;
  errors: string[];
}
