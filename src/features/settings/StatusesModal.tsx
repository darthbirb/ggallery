import { useEffect, useState } from "react";

import * as ipc from "../../lib/ipc";
import type { FolderStatusDef } from "../../lib/types";

interface StatusesModalProps {
  onClose: () => void;
  /** The sidebar's status dots and the folder header's picker both need
   *  refetching after an edit. */
  onChanged: () => void;
}

/**
 * Rename, recolour, reorder, add and remove folder status values — the app
 * ships a small unopinionated default set (active/wip/done/archived), fully
 * editable. Disposable scaffolding per §M2.1, M2.5 designs the real editor.
 */
export function StatusesModal({ onClose, onChanged }: StatusesModalProps) {
  const [statuses, setStatuses] = useState<FolderStatusDef[]>([]);
  const [newLabel, setNewLabel] = useState("");
  const [newColour, setNewColour] = useState("#6b7280");
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      setStatuses(await ipc.listFolderStatuses());
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  };

  useEffect(() => {
    void refresh();
  }, []);

  const run = async (action: () => Promise<void>) => {
    try {
      await action();
      await refresh();
      onChanged();
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  };

  const create = () => {
    const label = newLabel.trim();
    if (!label) return;
    void run(async () => {
      await ipc.createFolderStatus(label, newColour);
      setNewLabel("");
    });
  };

  const remove = (key: string) => {
    void (async () => {
      try {
        const count = await ipc.countFoldersByStatus(key);
        let reassignTo: string | null = null;
        if (count > 0) {
          const others = statuses.filter((s) => s.key !== key);
          if (others.length === 0) {
            setError("at least one folder status must remain");
            return;
          }
          const choice = prompt(
            `${count} folder(s) use this status. Reassign them to one of: ${others
              .map((s) => s.key)
              .join(", ")}`,
            others[0].key,
          );
          if (!choice) return;
          reassignTo = choice.trim();
        }
        await ipc.removeFolderStatus(key, reassignTo);
        await refresh();
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const move = (key: string, direction: -1 | 1) => {
    const keys = statuses.map((s) => s.key);
    const index = keys.indexOf(key);
    const swapWith = index + direction;
    if (swapWith < 0 || swapWith >= keys.length) return;
    [keys[index], keys[swapWith]] = [keys[swapWith], keys[index]];
    void run(() => ipc.reorderFolderStatuses(keys));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[82vh] w-[460px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Folder statuses</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto p-4 text-[13px]">
          {error && (
            <p className="mb-3 rounded-[3px] border border-danger/40 bg-raised px-3 py-2 text-danger">
              {error}
            </p>
          )}

          <table className="w-full border-collapse text-left">
            <tbody>
              {statuses.map((status, i) => (
                <tr key={status.key} className="border-t border-line-soft/60">
                  <td className="w-6 py-1.5">
                    <input
                      type="color"
                      value={status.colour}
                      onChange={(event) =>
                        void run(() => ipc.recolourFolderStatus(status.key, event.target.value))
                      }
                      className="h-4 w-4 border-none bg-transparent p-0"
                    />
                  </td>
                  <td className="py-1.5 pr-2">
                    <input
                      defaultValue={status.label}
                      key={status.key + status.label}
                      onBlur={(event) => {
                        const value = event.target.value.trim();
                        if (value && value !== status.label) {
                          void run(() => ipc.renameFolderStatus(status.key, value));
                        }
                      }}
                      className="w-full rounded-[3px] border border-transparent bg-transparent px-1 py-0.5 text-fg hover:border-line-soft focus:border-accent-d"
                    />
                  </td>
                  <td className="py-1.5 pr-2 font-mono text-fg-dim">{status.key}</td>
                  <td className="py-1.5 text-right">
                    <button
                      type="button"
                      disabled={i === 0}
                      onClick={() => move(status.key, -1)}
                      className="px-1 text-fg-dim hover:text-fg disabled:opacity-30"
                    >
                      ↑
                    </button>
                    <button
                      type="button"
                      disabled={i === statuses.length - 1}
                      onClick={() => move(status.key, 1)}
                      className="px-1 text-fg-dim hover:text-fg disabled:opacity-30"
                    >
                      ↓
                    </button>
                    <button
                      type="button"
                      onClick={() => remove(status.key)}
                      className="px-1 text-fg-dim hover:text-danger"
                    >
                      ×
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>

          <div className="mt-3 flex items-center gap-2">
            <input
              type="color"
              value={newColour}
              onChange={(event) => setNewColour(event.target.value)}
              className="h-5 w-5 border-none bg-transparent p-0"
            />
            <input
              value={newLabel}
              onChange={(event) => setNewLabel(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && create()}
              placeholder="+ new status"
              className="flex-1 rounded-[3px] border border-dashed border-line-soft bg-transparent px-1.5 py-0.5 text-fg placeholder:text-fg-dim"
            />
            <button
              type="button"
              onClick={create}
              className="rounded-[3px] border border-line px-2 py-0.5 text-fg-mid hover:bg-hover"
            >
              add
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
