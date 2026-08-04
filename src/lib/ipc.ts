/**
 * The only file that calls `invoke()`. Everything else calls typed functions,
 * so a backend signature change breaks at compile time in one place.
 */

import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

import type {
  ArchetypeFieldUsage,
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
  ItemDetail,
  LibraryInfo,
  LibraryStatus,
  MoveItemsReport,
  Progress,
  ReviewReport,
  ScanReport,
  TagSummary,
  TrashItemsReport,
  UndoReport,
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

/** One item in full — what the pane's Preview mode renders. */
export function getItem(itemId: number): Promise<ItemDetail> {
  return invoke<ItemDetail>("get_item", { itemId });
}

export function setItemsFavorite(
  itemIds: number[],
  favorite: boolean,
): Promise<void> {
  return invoke<void>("set_items_favorite", { itemIds, favorite });
}

// --- M2.5a: interface preferences, stored beside window geometry ---------

export function uiPrefs(): Promise<unknown> {
  return invoke<unknown>("ui_prefs");
}

export function setUiPrefs(prefs: unknown): Promise<void> {
  return invoke<void>("set_ui_prefs", { prefs });
}

/** Reverse one journalled batch — what a toast's Undo button calls. The
 *  `Ctrl+Z` stack replayer is still M4's; this reverses a named batch. */
export function undoBatch(batchId: string): Promise<UndoReport> {
  return invoke<UndoReport>("undo_batch", { batchId });
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

/** A folder has one name — this renames the directory to match whenever the
 *  sanitised title differs from what's on disk. There is no separate
 *  rename-directory call. */
export function setFolderTitle(id: number, title: string): Promise<string> {
  return invoke<string>("set_folder_title", { id, title });
}

/** Choose the folder's cover, or `null` to fall back to the automatic pick. */
export function setFolderCover(id: number, itemId: number | null): Promise<void> {
  return invoke<void>("set_folder_cover", { id, itemId });
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

/** Un-applies the current archetype and drops the field values it owned —
 *  a one-off field added independently is untouched. */
export function removeFolderArchetype(id: number): Promise<void> {
  return invoke<void>("remove_folder_archetype", { id });
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

/** No frontend caller yet — item-level tag UI is M2.5's. Exposed so M2.5 has
 *  a typed entry point to build on. */
export function itemEffectiveTags(itemId: number): Promise<EffectiveTag[]> {
  return invoke<EffectiveTag[]>("item_effective_tags", { itemId });
}

/** A folder's own labels and flags come back from `getFolder`; this is only
 *  the part it inherits from its ancestors — DESIGN.md §2's "inherited
 *  greyed, manual solid" rule, applied to a folder's own band the same way
 *  it already applies to an item's details. */
export function folderInheritedTags(folderId: number): Promise<EffectiveTag[]> {
  return invoke<EffectiveTag[]>("folder_inherited_tags", { folderId });
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

// --- M2.1: folder lifecycle ------------------------------------------------

export function createFolder(
  parentId: number | null,
  name: string,
  archetypeId: number | null,
): Promise<number> {
  return invoke<number>("create_folder", { parentId, name, archetypeId });
}

export function moveFolder(
  id: number,
  newParentId: number | null,
): Promise<string> {
  return invoke<string>("move_folder", { id, newParentId });
}

export function revealFolder(id: number): Promise<void> {
  return invoke<void>("reveal_folder", { id });
}

export function deleteFolder(id: number): Promise<string> {
  return invoke<string>("delete_folder", { id });
}

// --- M2.1: archetype lifecycle ----------------------------------------

export function createArchetype(name: string): Promise<number> {
  return invoke<number>("create_archetype", { name });
}

export function renameArchetype(id: number, name: string): Promise<void> {
  return invoke<void>("rename_archetype", { id, name });
}

export function deleteArchetype(id: number): Promise<void> {
  return invoke<void>("delete_archetype", { id });
}

export function countFoldersUsingArchetype(archetypeId: number): Promise<number> {
  return invoke<number>("count_folders_using_archetype", { archetypeId });
}

export function addArchetypeField(
  archetypeId: number,
  key: string,
  applyToExisting: boolean,
): Promise<void> {
  return invoke<void>("add_archetype_field", { archetypeId, key, applyToExisting });
}

export function reorderArchetypeFields(
  archetypeId: number,
  orderedKeys: string[],
): Promise<void> {
  return invoke<void>("reorder_archetype_fields", { archetypeId, orderedKeys });
}

export function archetypeFieldUsage(
  archetypeId: number,
  key: string,
): Promise<ArchetypeFieldUsage[]> {
  return invoke<ArchetypeFieldUsage[]>("archetype_field_usage", { archetypeId, key });
}

export function removeArchetypeField(archetypeId: number, key: string): Promise<void> {
  return invoke<void>("remove_archetype_field", { archetypeId, key });
}

// --- M2.1: folder status lifecycle -------------------------------------

export function createFolderStatus(label: string, colour: string): Promise<string> {
  return invoke<string>("create_folder_status", { label, colour });
}

export function renameFolderStatus(key: string, label: string): Promise<void> {
  return invoke<void>("rename_folder_status", { key, label });
}

export function recolourFolderStatus(key: string, colour: string): Promise<void> {
  return invoke<void>("recolour_folder_status", { key, colour });
}

export function reorderFolderStatuses(orderedKeys: string[]): Promise<void> {
  return invoke<void>("reorder_folder_statuses", { orderedKeys });
}

export function countFoldersByStatus(key: string): Promise<number> {
  return invoke<number>("count_folders_by_status", { key });
}

export function removeFolderStatus(
  key: string,
  reassignTo: string | null,
): Promise<void> {
  return invoke<void>("remove_folder_status", { key, reassignTo });
}

// --- M2.1: item move, delete, and OS-integration escape hatches --------

export function moveItems(
  itemIds: number[],
  destFolderId: number,
): Promise<MoveItemsReport> {
  return invoke<MoveItemsReport>("move_items", { itemIds, destFolderId });
}

export function deleteItems(itemIds: number[]): Promise<TrashItemsReport> {
  return invoke<TrashItemsReport>("delete_items", { itemIds });
}

export function revealItem(itemId: number): Promise<void> {
  return invoke<void>("reveal_item", { itemId });
}

export function openItem(itemId: number): Promise<void> {
  return invoke<void>("open_item", { itemId });
}

export function copyItemFile(itemId: number): Promise<void> {
  return invoke<void>("copy_item_file", { itemId });
}

export function copyItemPath(itemId: number): Promise<void> {
  return invoke<void>("copy_item_path", { itemId });
}

// --- M2.1: rename / delete a tag ----------------------------------------

export function listTags(filter: string | null): Promise<TagSummary[]> {
  return invoke<TagSummary[]>("list_tags", { filter });
}

export function renameTag(tagId: number, value: string): Promise<void> {
  return invoke<void>("rename_tag", { tagId, value });
}

export function deleteTag(tagId: number): Promise<void> {
  return invoke<void>("delete_tag", { tagId });
}

/** Absolute cache path to something the webview can load. */
export function assetUrl(directory: string, relative: string): string {
  return convertFileSrc(`${directory}/${relative}`);
}

/** The same, for a path the backend has already resolved in full — the
 *  original media file the preview shows. */
export function assetPath(absolute: string): string {
  return convertFileSrc(absolute);
}

/** Commands reject with `{ kind, message }`; anything else is a surprise. */
export function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error && typeof error === "object" && "message" in error) {
    return String((error as AppError).message);
  }
  return "Something went wrong.";
}

/** The folder a `"folder-missing"` error names, so a caller can offer
 *  removing the broken record — `null` for every other kind of failure. */
export function errorMissingFolder(error: unknown): { id: number; title: string } | null {
  if (error && typeof error === "object" && "folderId" in error) {
    const { folderId, folderTitle } = error as AppError;
    if (typeof folderId === "number" && typeof folderTitle === "string") {
      return { id: folderId, title: folderTitle };
    }
  }
  return null;
}
