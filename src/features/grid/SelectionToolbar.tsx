import { useState } from "react";

import * as ipc from "../../lib/ipc";
import type { FolderNode } from "../../lib/types";
import type { SelectionController } from "../../state/selection";
import { FolderPickerModal } from "../folder/FolderPickerModal";

interface SelectionToolbarProps {
  selection: SelectionController;
  folders: FolderNode[];
  /** The items list and the folder tree's counts both need refreshing after
   *  a move or delete. */
  onChanged: () => void;
}

/**
 * Selection count, select all/invert/clear, and the item operations that
 * act on the whole selection — move, delete, and (for exactly one item)
 * reveal/open/copy. Disposable scaffolding per PLAN.md §M2.1; M2.5 designs
 * where these controls actually live.
 */
export function SelectionToolbar({ selection, folders, onChanged }: SelectionToolbarProps) {
  const [showMove, setShowMove] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (selection.count === 0) return null;

  const ids = [...selection.selected];
  const singleId = ids.length === 1 ? ids[0] : null;

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(ipc.errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const doDelete = () => {
    const count = ids.length;
    if (!confirm(`Delete ${count} item${count === 1 ? "" : "s"}? They'll move to .gallery/trash.`)) {
      return;
    }
    void run(async () => {
      const report = await ipc.deleteItems(ids);
      if (report.errors.length > 0) {
        setError(`${report.errors.length} of ${count} item(s) could not be deleted.`);
      }
      selection.clear();
      onChanged();
    });
  };

  return (
    <div className="flex items-center gap-3 border-b border-line bg-panel px-3 py-1.5 text-[12px]">
      <span className="font-mono tabular-nums text-fg">{selection.count} selected</span>
      <button type="button" onClick={selection.selectAll} className="text-fg-dim hover:text-fg">
        select all
      </button>
      <button type="button" onClick={selection.invert} className="text-fg-dim hover:text-fg">
        invert
      </button>
      <button type="button" onClick={selection.clear} className="text-fg-dim hover:text-fg">
        clear
      </button>

      <span className="h-3 w-px bg-line-soft" />

      <button
        type="button"
        disabled={busy}
        onClick={() => setShowMove(true)}
        className="text-fg-dim hover:text-fg disabled:opacity-40"
      >
        move to…
      </button>
      <button
        type="button"
        disabled={busy}
        onClick={doDelete}
        className="text-danger hover:opacity-80 disabled:opacity-40"
      >
        delete
      </button>

      {singleId !== null && (
        <>
          <span className="h-3 w-px bg-line-soft" />
          <button
            type="button"
            disabled={busy}
            onClick={() => run(() => ipc.revealItem(singleId))}
            className="text-fg-dim hover:text-fg disabled:opacity-40"
          >
            reveal
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => run(() => ipc.openItem(singleId))}
            className="text-fg-dim hover:text-fg disabled:opacity-40"
          >
            open
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => run(() => ipc.copyItemFile(singleId))}
            className="text-fg-dim hover:text-fg disabled:opacity-40"
          >
            copy file
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={() => run(() => ipc.copyItemPath(singleId))}
            className="text-fg-dim hover:text-fg disabled:opacity-40"
          >
            copy path
          </button>
        </>
      )}

      {error && <span className="ml-auto truncate text-danger">{error}</span>}

      {showMove && (
        <FolderPickerModal
          folders={folders}
          onClose={() => setShowMove(false)}
          onPick={(destFolderId) => {
            setShowMove(false);
            void run(async () => {
              const report = await ipc.moveItems(ids, destFolderId);
              if (report.errors.length > 0) {
                setError(`${report.errors.length} of ${ids.length} item(s) could not be moved.`);
              }
              selection.clear();
              onChanged();
            });
          }}
        />
      )}
    </div>
  );
}
