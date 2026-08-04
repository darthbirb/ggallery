/**
 * Every mutating operation the interface can perform, in one place, each one
 * ending in a toast.
 *
 * Two rules from PLAN.md meet here. Decision 23: nothing is keyboard-only, and
 * every destructive action ends in a toast naming what happened with an Undo
 * button — which is also the only thing that makes the journal discoverable.
 * Decision 22: every noun has a full lifecycle, so create, rename and remove
 * are capabilities living here rather than descriptions of a menu item.
 *
 * The menus and dialogs in this folder call these; nothing calls `ipc`
 * directly for a mutation, so the toast can never be forgotten at a call site.
 */

import { createContext, createElement, useContext, useMemo, type ReactNode } from "react";

import * as ipc from "../../lib/ipc";
import type { FolderNode } from "../../lib/types";
import type { LibraryController } from "../../state/library";
import type { SelectionController } from "../../state/selection";
import { useToasts, type ToastQueue } from "../../state/toasts";

function plural(count: number, one: string, many?: string): string {
  return `${count} ${count === 1 ? one : (many ?? `${one}s`)}`;
}

export interface Operations {
  // --- items ---------------------------------------------------------
  moveItems: (itemIds: number[], dest: FolderNode) => Promise<void>;
  deleteItems: (itemIds: number[]) => Promise<void>;
  setFavorite: (itemIds: number[], favorite: boolean) => Promise<void>;
  revealItem: (itemId: number) => Promise<void>;
  openItem: (itemId: number) => Promise<void>;
  copyItemFile: (itemId: number) => Promise<void>;
  copyItemPath: (itemId: number) => Promise<void>;
  addItemTag: (itemIds: number[], key: string | null, value: string) => Promise<void>;
  removeItemTag: (itemId: number, tagId: number) => Promise<void>;
  setFolderCover: (folderId: number, itemId: number | null) => Promise<void>;

  // --- folders -------------------------------------------------------
  createFolder: (
    parentId: number | null,
    parentTitle: string,
    name: string,
    archetypeId: number | null,
  ) => Promise<void>;
  renameFolder: (folder: { id: number; title: string }, title: string) => Promise<void>;
  moveFolder: (folder: FolderNode, dest: FolderNode | null) => Promise<void>;
  deleteFolder: (folder: { id: number; title: string }) => Promise<void>;
  setFolderStatus: (folderId: number, status: string, label: string) => Promise<void>;
  setFolderFavorite: (folderId: number, title: string, favorite: boolean) => Promise<void>;
  setFolderNotes: (folderId: number, notes: string | null) => Promise<void>;
  setFolderLabel: (folderId: number, key: string, value: string) => Promise<void>;
  addFolderFlag: (folderId: number, value: string) => Promise<void>;
  removeFolderTag: (folderId: number, tagId: number) => Promise<void>;
  applyArchetype: (folderId: number, archetypeId: number, name: string) => Promise<void>;
  removeArchetype: (folderId: number) => Promise<void>;
  revealFolder: (folderId: number) => Promise<void>;
}

interface Deps {
  library: LibraryController;
  selection: SelectionController;
  toasts: ToastQueue;
}

/** Report a failure the same way as a success: in the strip, not a dialog. */
function fail(toasts: ToastQueue, what: string, err: unknown) {
  toasts.push({ message: `${what}: ${ipc.errorMessage(err)}`, tone: "danger" });
}

/** Undo for one journal batch, wired to the toast that names the operation. */
function undoFor(deps: Deps, batchId: string) {
  return async () => {
    const report = await ipc.undoBatch(batchId);
    if (report.errors.length > 0) {
      throw new Error(report.errors[0]);
    }
    deps.library.reload();
  };
}

export function buildOperations(deps: Deps): Operations {
  const { library, selection, toasts } = deps;

  return {
    async moveItems(itemIds, dest) {
      try {
        const report = await ipc.moveItems(itemIds, dest.id);
        library.reload();
        selection.clear();
        if (report.moved === 0 && report.errors.length > 0) {
          toasts.push({
            message: `Nothing moved — ${report.errors[0].error}`,
            tone: "danger",
          });
          return;
        }
        const failed =
          report.errors.length > 0
            ? ` (${plural(report.errors.length, "item")} could not be moved)`
            : "";
        toasts.push({
          // The destination *is* the outcome here, so it stays.
          message: `Moved ${plural(report.moved, "item")} to ${dest.title}${failed}`,
          undo: undoFor(deps, report.batchId),
          undoneMessage: `Moved ${plural(report.moved, "item")} back`,
        });
      } catch (err) {
        fail(toasts, "Could not move", err);
      }
    },

    async deleteItems(itemIds) {
      try {
        const report = await ipc.deleteItems(itemIds);
        library.reload();
        selection.clear();
        if (report.trashed === 0 && report.errors.length > 0) {
          toasts.push({
            message: `Nothing deleted — ${report.errors[0].error}`,
            tone: "danger",
          });
          return;
        }
        toasts.push({
          message: `Deleted ${plural(report.trashed, "item")}`,
          undo: undoFor(deps, report.batchId),
          undoneMessage: `Restored ${plural(report.trashed, "item")}`,
        });
      } catch (err) {
        fail(toasts, "Could not delete", err);
      }
    },

    async setFavorite(itemIds, favorite) {
      try {
        await ipc.setItemsFavorite(itemIds, favorite);
        library.reload();
        // Not destructive, and instantly reversible by the same control —
        // the badge on the tile is the feedback, so no toast.
      } catch (err) {
        fail(toasts, "Could not change favourite", err);
      }
    },

    async revealItem(itemId) {
      try {
        await ipc.revealItem(itemId);
      } catch (err) {
        fail(toasts, "Could not open Explorer", err);
      }
    },

    async openItem(itemId) {
      try {
        await ipc.openItem(itemId);
      } catch (err) {
        fail(toasts, "Could not open the file", err);
      }
    },

    async copyItemFile(itemId) {
      try {
        await ipc.copyItemFile(itemId);
        toasts.push({ message: "File copied" });
      } catch (err) {
        fail(toasts, "Could not copy the file", err);
      }
    },

    async copyItemPath(itemId) {
      try {
        await ipc.copyItemPath(itemId);
        toasts.push({ message: "Path copied" });
      } catch (err) {
        fail(toasts, "Could not copy the path", err);
      }
    },

    async addItemTag(itemIds, key, value) {
      try {
        for (const id of itemIds) {
          await ipc.addItemTag(id, key, value);
        }
        library.reload();
        toasts.push({
          message: `Tagged ${plural(itemIds.length, "item")} ${key ? `${key}: ${value}` : value}`,
        });
      } catch (err) {
        fail(toasts, "Could not add the tag", err);
      }
    },

    async removeItemTag(itemId, tagId) {
      try {
        await ipc.removeItemTag(itemId, tagId);
        library.reload();
      } catch (err) {
        fail(toasts, "Could not remove the tag", err);
      }
    },

    async setFolderCover(folderId, itemId) {
      try {
        await ipc.setFolderCover(folderId, itemId);
        library.refreshFolders();
        toasts.push({
          message: itemId === null ? "Cover cleared" : "Cover set",
        });
      } catch (err) {
        fail(toasts, "Could not set the cover", err);
      }
    },

    async createFolder(parentId, parentTitle, name, archetypeId) {
      try {
        await ipc.createFolder(parentId, name, archetypeId);
        library.refreshFolders();
        toasts.push({ message: `Created ${name} in ${parentTitle}` });
      } catch (err) {
        fail(toasts, "Could not create the folder", err);
      }
    },

    async renameFolder(folder, title) {
      if (title === folder.title) return;
      try {
        const batchId = await ipc.setFolderTitle(folder.id, title);
        library.reload();
        toasts.push({
          // A folder has one name: the directory on disk follows the title.
          message: `Renamed ${folder.title} to ${title}`,
          undo: undoFor(deps, batchId),
          undoneMessage: `Renamed back to ${folder.title}`,
        });
      } catch (err) {
        fail(toasts, "Could not rename the folder", err);
      }
    },

    async moveFolder(folder, dest) {
      try {
        const batchId = await ipc.moveFolder(folder.id, dest?.id ?? null);
        library.reload();
        toasts.push({
          message: `Moved ${folder.title} to ${dest ? dest.title : "the top level"}`,
          undo: undoFor(deps, batchId),
          undoneMessage: `Moved ${folder.title} back`,
        });
      } catch (err) {
        fail(toasts, "Could not move the folder", err);
      }
    },

    async deleteFolder(folder) {
      try {
        const batchId = await ipc.deleteFolder(folder.id);
        library.reload();
        toasts.push({
          message: `Deleted ${folder.title}`,
          undo: undoFor(deps, batchId),
          undoneMessage: `Restored ${folder.title}`,
        });
      } catch (err) {
        fail(toasts, "Could not delete the folder", err);
      }
    },

    async setFolderStatus(folderId, status, label) {
      try {
        await ipc.setFolderStatus(folderId, status);
        library.refreshFolders();
        toasts.push({ message: `Status set to ${label}` });
      } catch (err) {
        fail(toasts, "Could not set the status", err);
      }
    },

    async setFolderFavorite(folderId, title, favorite) {
      try {
        await ipc.setFolderFavorite(folderId, favorite);
        library.refreshFolders();
        toasts.push({
          message: favorite ? `Pinned ${title}` : `Unpinned ${title}`,
        });
      } catch (err) {
        fail(toasts, "Could not pin the folder", err);
      }
    },

    async setFolderNotes(folderId, notes) {
      try {
        await ipc.setFolderNotes(folderId, notes);
      } catch (err) {
        fail(toasts, "Could not save the notes", err);
      }
    },

    async setFolderLabel(folderId, key, value) {
      try {
        await ipc.setFolderLabel(folderId, key, value);
        library.refreshFolders();
      } catch (err) {
        fail(toasts, "Could not save the field", err);
      }
    },

    async addFolderFlag(folderId, value) {
      try {
        await ipc.addFolderFlag(folderId, value);
        library.refreshFolders();
      } catch (err) {
        fail(toasts, "Could not add the tag", err);
      }
    },

    async removeFolderTag(folderId, tagId) {
      try {
        await ipc.removeFolderTag(folderId, tagId);
        library.refreshFolders();
      } catch (err) {
        fail(toasts, "Could not remove the tag", err);
      }
    },

    async applyArchetype(folderId, archetypeId, name) {
      try {
        await ipc.applyFolderArchetype(folderId, archetypeId);
        library.refreshFolders();
        toasts.push({ message: `Applied ${name}` });
      } catch (err) {
        fail(toasts, "Could not apply the archetype", err);
      }
    },

    async removeArchetype(folderId) {
      try {
        await ipc.removeFolderArchetype(folderId);
        library.refreshFolders();
        toasts.push({ message: "Removed the archetype" });
      } catch (err) {
        fail(toasts, "Could not remove the archetype", err);
      }
    },

    async revealFolder(folderId) {
      try {
        await ipc.revealFolder(folderId);
      } catch (err) {
        fail(toasts, "Could not open Explorer", err);
      }
    },
  };
}

const OperationsContext = createContext<Operations | null>(null);

export function OperationsProvider({
  library,
  selection,
  children,
}: {
  library: LibraryController;
  selection: SelectionController;
  children: ReactNode;
}) {
  const toasts = useToasts();
  const value = useMemo(
    () => buildOperations({ library, selection, toasts }),
    [library, selection, toasts],
  );
  return createElement(OperationsContext.Provider, { value }, children);
}

export function useOperations(): Operations {
  const value = useContext(OperationsContext);
  if (!value) {
    throw new Error("useOperations must be used inside <OperationsProvider>");
  }
  return value;
}
