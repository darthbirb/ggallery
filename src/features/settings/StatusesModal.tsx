import { useCallback, useEffect, useState } from "react";

import { Button, IconButton } from "../../components/Button";
import { Dialog } from "../../components/Dialog";
import * as ipc from "../../lib/ipc";
import type { FolderStatusDef } from "../../lib/types";

interface StatusesModalProps {
  onClose: () => void;
  /** The tree's WIP mark and the band's status chip both need refetching
   *  after an edit. */
  onChanged: () => void;
}

/**
 * Rename, recolour, reorder, add and remove folder status values — the app
 * ships a small unopinionated default set, fully editable (docs/DESIGN.md §1
 * "Folder status"; decision 22's full lifecycle).
 *
 * Removing a status that folders are using asks where those folders should
 * go, by name, in a dialog rather than a browser prompt.
 */
export function StatusesModal({ onClose, onChanged }: StatusesModalProps) {
  const [statuses, setStatuses] = useState<FolderStatusDef[]>([]);
  const [newLabel, setNewLabel] = useState("");
  const [newColour, setNewColour] = useState("#6b7280");
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<{
    status: FolderStatusDef;
    count: number;
  } | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatuses(await ipc.listFolderStatuses());
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = async (action: () => Promise<void>) => {
    try {
      setError(null);
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

  const askRemove = (status: FolderStatusDef) => {
    void (async () => {
      try {
        const count = await ipc.countFoldersByStatus(status.key);
        if (count > 0 && statuses.length === 1) {
          setError("At least one folder status has to remain.");
          return;
        }
        if (count === 0) {
          await run(() => ipc.removeFolderStatus(status.key, null));
          return;
        }
        setRemoving({ status, count });
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const move = (key: string, direction: -1 | 1) => {
    const keys = statuses.map((status) => status.key);
    const index = keys.indexOf(key);
    const swapWith = index + direction;
    if (swapWith < 0 || swapWith >= keys.length) return;
    [keys[index], keys[swapWith]] = [keys[swapWith], keys[index]];
    void run(() => ipc.reorderFolderStatuses(keys));
  };

  return (
    <>
      <Dialog
        open
        onOpenChange={(open) => !open && onClose()}
        title="Folder statuses"
        description="Rename, recolour, reorder, add or remove. One of them marks folders in the tree — see below."
        width={500}
      >
        {error && (
          <p className="mb-3 rounded-[4px] border border-danger/40 bg-raised px-3 py-2 text-danger">
            {error}
          </p>
        )}

        <table className="w-full border-collapse text-left text-[13px]">
          <tbody>
            {statuses.map((status, index) => (
              <tr key={status.key} className="border-t border-line-soft/60">
                <td className="w-6 py-1.5">
                  <input
                    type="color"
                    aria-label={`${status.label} colour`}
                    value={status.colour}
                    onChange={(event) =>
                      void run(() =>
                        ipc.recolourFolderStatus(status.key, event.target.value),
                      )
                    }
                    className="h-4 w-4 border-none bg-transparent p-0"
                  />
                </td>
                <td className="py-1.5 pr-2">
                  <input
                    defaultValue={status.label}
                    key={status.key + status.label}
                    aria-label={`${status.label} name`}
                    onBlur={(event) => {
                      const value = event.target.value.trim();
                      if (value && value !== status.label) {
                        void run(() => ipc.renameFolderStatus(status.key, value));
                      }
                    }}
                    className="w-full rounded-[3px] border border-transparent bg-transparent px-1 py-0.5 text-fg hover:border-line-soft focus:border-accent-d"
                  />
                </td>
                <td className="py-1.5 pr-2 font-mono text-[11px] text-fg-dim">
                  {status.key}
                </td>
                <td className="py-1.5 text-right">
                  <IconButton
                    aria-label={`Move ${status.label} up`}
                    disabled={index === 0}
                    onClick={() => move(status.key, -1)}
                  >
                    ↑
                  </IconButton>
                  <IconButton
                    aria-label={`Move ${status.label} down`}
                    disabled={index === statuses.length - 1}
                    onClick={() => move(status.key, 1)}
                  >
                    ↓
                  </IconButton>
                  <IconButton
                    aria-label={`Remove ${status.label}`}
                    onClick={() => askRemove(status)}
                  >
                    ×
                  </IconButton>
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        <div className="mt-3 flex items-center gap-2">
          <input
            type="color"
            aria-label="New status colour"
            value={newColour}
            onChange={(event) => setNewColour(event.target.value)}
            className="h-5 w-5 border-none bg-transparent p-0"
          />
          <input
            value={newLabel}
            aria-label="New status name"
            onChange={(event) => setNewLabel(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && create()}
            placeholder="＋ new status"
            className="flex-1 rounded-[4px] border border-dashed border-line bg-transparent px-1.5 py-0.5 text-[13px] text-fg placeholder:text-fg-dim focus:border-accent-d"
          />
          <Button variant="outline" onClick={create}>
            Add
          </Button>
        </div>

        <p className="mt-3 text-[12px] text-fg-dim">
          The tree marks one status with a single dot and nothing for the rest —
          one mark meaning &ldquo;needs more&rdquo;, so it is glanceable without
          a legend.
        </p>
      </Dialog>

      {removing && (
        <ReassignDialog
          status={removing.status}
          count={removing.count}
          others={statuses.filter((status) => status.key !== removing.status.key)}
          onClose={() => setRemoving(null)}
          onConfirm={(reassignTo) => {
            setRemoving(null);
            void run(() => ipc.removeFolderStatus(removing.status.key, reassignTo));
          }}
        />
      )}
    </>
  );
}

/** Removing a status that folders are using never silently strands them. */
function ReassignDialog({
  status,
  count,
  others,
  onClose,
  onConfirm,
}: {
  status: FolderStatusDef;
  count: number;
  others: FolderStatusDef[];
  onClose: () => void;
  onConfirm: (reassignTo: string) => void;
}) {
  const [choice, setChoice] = useState(others[0]?.key ?? "");

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title={`Remove ${status.label}?`}
      width={420}
      footer={
        <>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="danger" disabled={!choice} onClick={() => onConfirm(choice)}>
            Remove and reassign
          </Button>
        </>
      }
    >
      <p className="mb-2 text-[13px] text-fg-mid">
        {count === 1 ? "1 folder uses" : `${count} folders use`} this status. Give
        them another one instead:
      </p>
      <select
        autoFocus
        aria-label="Reassign to"
        value={choice}
        onChange={(event) => setChoice(event.target.value)}
        className="w-full rounded-[4px] border border-line bg-ground px-2 py-1 text-[13px] text-fg"
      >
        {others.map((other) => (
          <option key={other.key} value={other.key}>
            {other.label}
          </option>
        ))}
      </select>
    </Dialog>
  );
}
