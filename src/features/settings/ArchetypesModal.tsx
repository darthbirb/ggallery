import { useCallback, useEffect, useState } from "react";

import { Button, IconButton } from "../../components/Button";
import { ConfirmDialog, Dialog } from "../../components/Dialog";
import * as ipc from "../../lib/ipc";
import type { ArchetypeFieldUsage, ArchetypeInfo } from "../../lib/types";

interface ArchetypesModalProps {
  onClose: () => void;
  /** The folder band shows archetype fields — refetch after any edit. */
  onChanged: () => void;
}

/** `handle` is text matched with or without a leading `@`. It carries no
 *  knowledge of any platform and does not auto-link — decision 21. */
const FIELD_TYPES = ["text", "handle", "url", "date", "number"];

/**
 * Create, rename and delete an archetype; add, reorder and remove its fields.
 * Mandatory now that nothing is seeded (PLAN.md decision 21): with no default
 * vocabulary, an editor is the only way to have one at all.
 *
 * Two edits are never silent, per docs/DESIGN.md §1 "Archetypes": adding a
 * field asks whether folders already using the archetype should get it, and
 * removing one that holds values names the folders it would empty.
 */
export function ArchetypesModal({ onClose, onChanged }: ArchetypesModalProps) {
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [newName, setNewName] = useState("");
  const [newFieldKey, setNewFieldKey] = useState("");
  const [newFieldType, setNewFieldType] = useState(FIELD_TYPES[0]);
  const [error, setError] = useState<string | null>(null);
  const [removingField, setRemovingField] = useState<{
    key: string;
    usage: ArchetypeFieldUsage[];
  } | null>(null);
  const [addingField, setAddingField] = useState<{
    key: string;
    type: string;
    count: number;
  } | null>(null);
  const [deletingArchetype, setDeletingArchetype] = useState<ArchetypeInfo | null>(null);

  const refresh = useCallback(async () => {
    try {
      setArchetypes(await ipc.listArchetypes());
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

  const selected = archetypes.find((archetype) => archetype.id === selectedId) ?? null;

  const createArchetype = () => {
    const name = newName.trim();
    if (!name) return;
    void run(async () => {
      const id = await ipc.createArchetype(name);
      setNewName("");
      setSelectedId(id);
    });
  };

  const askRemoveField = (key: string) => {
    if (!selected) return;
    void (async () => {
      try {
        const usage = await ipc.archetypeFieldUsage(selected.id, key);
        if (usage.length === 0) {
          await run(() => ipc.removeArchetypeField(selected.id, key));
          return;
        }
        setRemovingField({ key, usage });
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const askAddField = () => {
    if (!selected) return;
    const key = newFieldKey.trim();
    if (!key) return;
    void (async () => {
      try {
        const count = await ipc.countFoldersUsingArchetype(selected.id);
        if (count === 0) {
          await run(async () => {
            await ipc.addArchetypeField(selected.id, key, newFieldType, false);
            setNewFieldKey("");
          });
          return;
        }
        setAddingField({ key, type: newFieldType, count });
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const addField = (applyToExisting: boolean) => {
    if (!selected || !addingField) return;
    const { key, type } = addingField;
    setAddingField(null);
    void run(async () => {
      await ipc.addArchetypeField(selected.id, key, type, applyToExisting);
      setNewFieldKey("");
    });
  };

  const moveField = (key: string, direction: -1 | 1) => {
    if (!selected) return;
    const keys = selected.fields.map((field) => field.key);
    const index = keys.indexOf(key);
    const swapWith = index + direction;
    if (swapWith < 0 || swapWith >= keys.length) return;
    [keys[index], keys[swapWith]] = [keys[swapWith], keys[index]];
    void run(() => ipc.reorderArchetypeFields(selected.id, keys));
  };

  return (
    <>
      <Dialog
        open
        onOpenChange={(open) => !open && onClose()}
        title="Archetypes"
        description="A named set of fields a folder can be given. The app ships with none — these are yours."
        width={640}
      >
        <div className="flex min-h-[320px] gap-3 text-[13px]">
          <div className="flex w-[180px] shrink-0 flex-col gap-1 border-r border-line pr-2">
            {archetypes.map((archetype) => (
              <button
                key={archetype.id}
                type="button"
                onClick={() => setSelectedId(archetype.id)}
                className={`w-full rounded-[4px] px-2 py-1 text-left ${
                  archetype.id === selectedId
                    ? "bg-accent/15 text-accent"
                    : "text-fg-mid hover:bg-hover hover:text-fg"
                }`}
              >
                {archetype.name}
              </button>
            ))}
            {archetypes.length === 0 && (
              <p className="px-1 py-1 text-fg-dim">None yet.</p>
            )}
            <input
              value={newName}
              aria-label="New archetype name"
              onChange={(event) => setNewName(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && createArchetype()}
              placeholder="＋ new archetype"
              className="mt-1 w-full rounded-[4px] border border-dashed border-line bg-transparent px-1.5 py-0.5 text-fg placeholder:text-fg-dim focus:border-accent-d"
            />
          </div>

          <div className="min-w-0 flex-1">
            {error && (
              <p className="mb-3 rounded-[4px] border border-danger/40 bg-raised px-3 py-2 text-danger">
                {error}
              </p>
            )}

            {!selected ? (
              <p className="text-fg-dim">Select or create an archetype.</p>
            ) : (
              <div className="flex flex-col gap-3">
                <div className="flex items-center gap-2">
                  <input
                    defaultValue={selected.name}
                    key={selected.id}
                    aria-label="Archetype name"
                    onBlur={(event) => {
                      const value = event.target.value.trim();
                      if (value && value !== selected.name) {
                        void run(() => ipc.renameArchetype(selected.id, value));
                      }
                    }}
                    className="flex-1 rounded-[4px] border border-line bg-ground px-1.5 py-1 font-semibold text-fg focus:border-accent-d"
                  />
                  <Button
                    variant="danger"
                    onClick={() => setDeletingArchetype(selected)}
                  >
                    Delete
                  </Button>
                </div>

                <table className="w-full border-collapse text-left">
                  <thead>
                    <tr className="font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
                      <th className="py-1 pr-2 font-normal">key</th>
                      <th className="py-1 pr-2 font-normal">type</th>
                      <th className="py-1"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {selected.fields.map((field, index) => (
                      <tr key={field.key} className="border-t border-line-soft/60">
                        <td className="py-1 pr-2 font-mono">{field.key}</td>
                        <td className="py-1 pr-2 text-fg-dim">{field.type}</td>
                        <td className="py-1 text-right">
                          <IconButton
                            aria-label={`Move ${field.key} up`}
                            disabled={index === 0}
                            onClick={() => moveField(field.key, -1)}
                          >
                            ↑
                          </IconButton>
                          <IconButton
                            aria-label={`Move ${field.key} down`}
                            disabled={index === selected.fields.length - 1}
                            onClick={() => moveField(field.key, 1)}
                          >
                            ↓
                          </IconButton>
                          <IconButton
                            aria-label={`Remove ${field.key}`}
                            onClick={() => askRemoveField(field.key)}
                          >
                            ×
                          </IconButton>
                        </td>
                      </tr>
                    ))}
                    {selected.fields.length === 0 && (
                      <tr>
                        <td colSpan={3} className="py-1.5 text-fg-dim">
                          No fields yet.
                        </td>
                      </tr>
                    )}
                  </tbody>
                </table>

                <div className="flex items-center gap-2">
                  <input
                    value={newFieldKey}
                    aria-label="New field key"
                    onChange={(event) => setNewFieldKey(event.target.value)}
                    onKeyDown={(event) => event.key === "Enter" && askAddField()}
                    placeholder="field key"
                    className="flex-1 rounded-[4px] border border-line bg-ground px-1.5 py-0.5 text-fg focus:border-accent-d"
                  />
                  <select
                    value={newFieldType}
                    aria-label="New field type"
                    onChange={(event) => setNewFieldType(event.target.value)}
                    className="rounded-[4px] border border-line bg-ground px-1 py-0.5 text-fg-mid"
                  >
                    {FIELD_TYPES.map((type) => (
                      <option key={type} value={type}>
                        {type}
                      </option>
                    ))}
                  </select>
                  <Button variant="outline" onClick={askAddField}>
                    Add field
                  </Button>
                </div>
              </div>
            )}
          </div>
        </div>
      </Dialog>

      {addingField && (
        <Dialog
          open
          onOpenChange={(open) => !open && setAddingField(null)}
          title={`Add ${addingField.key} to existing folders?`}
          width={430}
          footer={
            <>
              <Button variant="outline" onClick={() => addField(false)}>
                Just the archetype
              </Button>
              <Button variant="accent" onClick={() => addField(true)}>
                Add to all {addingField.count}
              </Button>
            </>
          }
        >
          <p className="text-[13px] text-fg-mid">
            {addingField.count === 1 ? "1 folder uses" : `${addingField.count} folders use`}{" "}
            this archetype. The field can be created empty on all of them now, or
            only apply to folders given the archetype later.
          </p>
        </Dialog>
      )}

      {removingField && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setRemovingField(null)}
          title={`Remove ${removingField.key}?`}
          body={`This deletes the value it holds on: ${removingField.usage
            .map((folder) => folder.title)
            .join(", ")}.`}
          confirmLabel="Remove the field"
          danger
          onConfirm={() => {
            const key = removingField.key;
            setRemovingField(null);
            if (selected) void run(() => ipc.removeArchetypeField(selected.id, key));
          }}
        />
      )}

      {deletingArchetype && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setDeletingArchetype(null)}
          title={`Delete ${deletingArchetype.name}?`}
          body="Folders using it keep the labels they already have — only the template goes."
          confirmLabel="Delete archetype"
          danger
          onConfirm={() => {
            const id = deletingArchetype.id;
            setDeletingArchetype(null);
            setSelectedId(null);
            void run(() => ipc.deleteArchetype(id));
          }}
        />
      )}
    </>
  );
}
