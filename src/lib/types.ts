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
