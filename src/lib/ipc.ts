/**
 * The only file that calls `invoke()`. Everything else calls typed functions,
 * so a backend signature change breaks at compile time in one place.
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  ArchetypeInfo,
  AppError,
  DryRunReport,
  EffectiveTag,
  ExecuteReport,
  FolderDetail,
  FolderNode,
  FolderStatusDef,
  FsExecuteReport,
  GridItem,
  ImportProgress,
  IndexFailure,
  LibraryInfo,
  LibraryStatus,
  NameParseCandidate,
  Progress,
  ReviewReport,
  ScanReport,
  VerifyReport,
} from "./types";

const PROGRESS_EVENT = "job-progress";
const IMPORT_PROGRESS_EVENT = "import-progress";

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

// --- M1.5 import wizard ------------------------------------------------

export function scanImport(): Promise<ScanReport> {
  return invoke<ScanReport>("scan_import");
}

export function dryRunImport(sampleSize: number): Promise<DryRunReport> {
  return invoke<DryRunReport>("dry_run_import", { sampleSize });
}

export function executeImport(confirmedBackup: boolean): Promise<ExecuteReport> {
  return invoke<ExecuteReport>("execute_import", { confirmedBackup });
}

export function verifyImport(sampleSize: number): Promise<VerifyReport> {
  return invoke<VerifyReport>("verify_import", { sampleSize });
}

/** For a library with nothing to rename — stamps it imported without ever
 *  showing the wizard. Not gated: nothing destructive happens. */
export function markImported(): Promise<void> {
  return invoke<void>("mark_imported");
}

export function onImportProgress(
  handler: (progress: ImportProgress) => void,
): Promise<UnlistenFn> {
  return listen<ImportProgress>(IMPORT_PROGRESS_EVENT, (event) =>
    handler(event.payload),
  );
}

// --- M1.7 startup flow ------------------------------------------------

/** Choose folder → this. Filesystem-only — no library is opened yet. */
export function prepareImport(path: string): Promise<ReviewReport> {
  return invoke<ReviewReport>("prepare_import", { path });
}

export function executePreparedImport(
  confirmedBackup: boolean,
): Promise<FsExecuteReport> {
  return invoke<FsExecuteReport>("execute_prepared_import", {
    confirmedBackup,
  });
}

/** Review → Cancel. Discards the staged plan; nothing to undo on disk. */
export function cancelPreparedImport(): Promise<void> {
  return invoke<void>("cancel_prepared_import");
}

// --- M2: folders as entities ------------------------------------------

export function getFolder(id: number): Promise<FolderDetail> {
  return invoke<FolderDetail>("get_folder", { id });
}

export function setFolderTitle(id: number, title: string): Promise<void> {
  return invoke<void>("set_folder_title", { id, title });
}

export function setFolderStatus(id: number, status: string): Promise<void> {
  return invoke<void>("set_folder_status", { id, status });
}

export function setFolderFavorite(id: number, favorite: boolean): Promise<void> {
  return invoke<void>("set_folder_favorite", { id, favorite });
}

export function setFolderNotes(id: number, notes: string | null): Promise<void> {
  return invoke<void>("set_folder_notes", { id, notes });
}

export function applyFolderArchetype(id: number, archetypeId: number): Promise<void> {
  return invoke<void>("apply_folder_archetype", { id, archetypeId });
}

export function setFolderLabel(
  id: number,
  key: string,
  value: string,
): Promise<void> {
  return invoke<void>("set_folder_label", { id, key, value });
}

export function addFolderFlag(id: number, value: string): Promise<void> {
  return invoke<void>("add_folder_flag", { id, value });
}

export function removeFolderTag(id: number, tagId: number): Promise<void> {
  return invoke<void>("remove_folder_tag", { id, tagId });
}

export function listFolderStatuses(): Promise<FolderStatusDef[]> {
  return invoke<FolderStatusDef[]>("list_folder_statuses");
}

export function listArchetypes(): Promise<ArchetypeInfo[]> {
  return invoke<ArchetypeInfo[]>("list_archetypes");
}

export function scanFolderNameParse(): Promise<NameParseCandidate[]> {
  return invoke<NameParseCandidate[]>("scan_folder_name_parse");
}

export function applyFolderNameParse(
  rows: NameParseCandidate[],
): Promise<void> {
  return invoke<void>("apply_folder_name_parse", { rows });
}

/** No frontend caller yet — item-level tag UI is M2.5's. Exposed so M2.5 has
 *  a typed entry point to build on. */
export function itemEffectiveTags(itemId: number): Promise<EffectiveTag[]> {
  return invoke<EffectiveTag[]>("item_effective_tags", { itemId });
}

export function addItemTag(
  itemId: number,
  key: string | null,
  value: string,
): Promise<void> {
  return invoke<void>("add_item_tag", { itemId, key, value });
}

export function removeItemTag(itemId: number, tagId: number): Promise<void> {
  return invoke<void>("remove_item_tag", { itemId, tagId });
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
