/**
 * The only file that calls `invoke()`. Everything else calls typed functions,
 * so a backend signature change breaks at compile time in one place.
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  AppError,
  FolderNode,
  GridItem,
  IndexFailure,
  LibraryInfo,
  LibraryStatus,
  Progress,
} from "./types";

const PROGRESS_EVENT = "job-progress";

export async function pickLibraryFolder(): Promise<string | null> {
  const picked = await open({
    directory: true,
    multiple: false,
    title: "Choose your library folder",
  });
  return typeof picked === "string" ? picked : null;
}

export function openLibrary(path: string): Promise<LibraryInfo> {
  return invoke<LibraryInfo>("open_library", { path });
}

export function currentLibrary(): Promise<LibraryStatus> {
  return invoke<LibraryStatus>("current_library");
}

export function closeLibrary(): Promise<void> {
  return invoke<void>("close_library");
}

export function folderTree(): Promise<FolderNode[]> {
  return invoke<FolderNode[]>("folder_tree");
}

export function listItems(
  folder: string | null,
  recursive: boolean,
): Promise<GridItem[]> {
  return invoke<GridItem[]>("list_items", { folder, recursive });
}

export function startIndex(): Promise<void> {
  return invoke<void>("start_index");
}

export function indexProgress(): Promise<Progress> {
  return invoke<Progress>("index_progress");
}

export function indexFailures(): Promise<IndexFailure[]> {
  return invoke<IndexFailure[]>("index_failures");
}

export function retryFailedJobs(): Promise<number> {
  return invoke<number>("retry_failed_jobs");
}

/** Queue progress, coalesced to one event per tick by the backend. */
export function onProgress(
  handler: (progress: Progress) => void,
): Promise<UnlistenFn> {
  return listen<Progress>(PROGRESS_EVENT, (event) => handler(event.payload));
}

/** Absolute cache path to something the webview can load. */
export function assetUrl(directory: string, relative: string): string {
  return convertFileSrc(`${directory}/${relative}`);
}

/** Commands reject with `{ kind, message }`; anything else is a surprise. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as AppError).message);
  }
  return "Something went wrong.";
}
