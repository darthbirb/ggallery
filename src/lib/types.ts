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
