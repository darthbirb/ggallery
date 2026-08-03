import { ArrowDown, ArrowUp, X } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { ConfirmDialog, Dialog } from "../../components/Dialog";
import { Button, IconButton } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import * as ipc from "../../lib/ipc";
import type { ArchetypeFieldUsage, ArchetypeInfo } from "../../lib/types";
import { cn } from "../../lib/utils";

interface ArchetypesSectionProps {
  /** The folder band shows archetype fields — refetch after any edit. */
  onChanged: () => void;
}

/**
 * Create, rename and delete an archetype; add, reorder and remove its fields.
 * Mandatory now that nothing is seeded (PLAN.md decision 21): with no default
 * vocabulary, an editor is the only way to have one at all.
 *
 * **A field is a name and a position.** It carried a type until M2.5a.1 —
 * text / handle / url / date / number — which asked the user a question the
 * app then ignored: decision 21 had already removed the platform linking
 * `handle` existed for, and nothing else ever read it.
 *
 * Two edits are never silent, per docs/DESIGN.md §1 "Archetypes": adding a
 * field asks whether folders already using the archetype should get it, and
 * removing one that holds values names the folders it would empty.
 *
 * A section inside `SettingsPanel`'s single dialog, not a dialog of its own —
 * see the comment there. The two prompts above stay real dialogs of their
 * own; they are transient confirmations, not another screen to navigate to.
 */
export function ArchetypesSection({ onChanged }: ArchetypesSectionProps) {
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [newName, setNewName] = useState("");
  const [newFieldKey, setNewFieldKey] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [removingField, setRemovingField] = useState<{
    key: string;
    usage: ArchetypeFieldUsage[];
  } | null>(null);
  const [addingField, setAddingField] = useState<{
    key: string;
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
            await ipc.addArchetypeField(selected.id, key, false);
            setNewFieldKey("");
          });
          return;
        }
        setAddingField({ key, count });
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const addField = (applyToExisting: boolean) => {
    if (!selected || !addingField) return;
    const { key } = addingField;
    setAddingField(null);
    void run(async () => {
      await ipc.addArchetypeField(selected.id, key, applyToExisting);
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
      <div className="flex min-h-[380px] gap-4">
        <div className="flex w-[190px] shrink-0 flex-col gap-1 border-r border-line pr-3">
          {archetypes.map((archetype) => (
            <button
              key={archetype.id}
              type="button"
              onClick={() => setSelectedId(archetype.id)}
              className={cn(
                "h-8 w-full rounded-[4px] border px-2 text-left",
                archetype.id === selectedId
                  ? "border-accent-d bg-accent/15 text-accent"
                  : "border-transparent text-fg-mid hover:bg-hover hover:text-fg",
              )}
            >
              {archetype.name}
            </button>
          ))}
          {archetypes.length === 0 && (
            <p className="px-1 py-1 text-fg-dim">None yet.</p>
          )}
          <Input
            value={newName}
            aria-label="New archetype name"
            onChange={(event) => setNewName(event.target.value)}
            onKeyDown={(event) => event.key === "Enter" && createArchetype()}
            placeholder="＋ new archetype"
            className="mt-1 border-dashed bg-transparent"
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
                <Input
                  defaultValue={selected.name}
                  key={selected.id}
                  aria-label="Archetype name"
                  onBlur={(event) => {
                    const value = event.target.value.trim();
                    if (value && value !== selected.name) {
                      void run(() => ipc.renameArchetype(selected.id, value));
                    }
                  }}
                  className="flex-1 font-semibold"
                />
                <Button variant="danger" onClick={() => setDeletingArchetype(selected)}>
                  Delete
                </Button>
              </div>

              <table className="w-full border-collapse text-left">
                <thead>
                  <tr className="font-mono uppercase tracking-[0.1em] text-fg-dim">
                    <th className="py-1 pr-2 font-normal">key</th>
                    <th className="py-1"></th>
                  </tr>
                </thead>
                <tbody>
                  {selected.fields.map((field, index) => (
                    <tr key={field.key} className="border-t border-line-soft/60">
                      <td className="py-1 pr-2 font-mono">{field.key}</td>
                      <td className="py-1">
                        <span className="flex items-center justify-end gap-1">
                          <IconButton
                            aria-label={`Move ${field.key} up`}
                            disabled={index === 0}
                            onClick={() => moveField(field.key, -1)}
                          >
                            <ArrowUp />
                          </IconButton>
                          <IconButton
                            aria-label={`Move ${field.key} down`}
                            disabled={index === selected.fields.length - 1}
                            onClick={() => moveField(field.key, 1)}
                          >
                            <ArrowDown />
                          </IconButton>
                          <IconButton
                            aria-label={`Remove ${field.key}`}
                            variant="danger"
                            onClick={() => askRemoveField(field.key)}
                          >
                            <X />
                          </IconButton>
                        </span>
                      </td>
                    </tr>
                  ))}
                  {selected.fields.length === 0 && (
                    <tr>
                      <td colSpan={2} className="py-1.5 text-fg-dim">
                        No fields yet.
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>

              <div className="flex items-center gap-2">
                <Input
                  value={newFieldKey}
                  aria-label="New field key"
                  onChange={(event) => setNewFieldKey(event.target.value)}
                  onKeyDown={(event) => event.key === "Enter" && askAddField()}
                  placeholder="field key"
                  className="flex-1"
                />
                <Button onClick={askAddField}>Add field</Button>
              </div>
            </div>
          )}
        </div>
      </div>

      {addingField && (
        <Dialog
          open
          onOpenChange={(open) => !open && setAddingField(null)}
          title={`Add ${addingField.key} to existing folders?`}
          width={430}
          footer={
            <>
              <Button onClick={() => addField(false)}>Just the archetype</Button>
              <Button variant="accent" onClick={() => addField(true)}>
                Add to all {addingField.count}
              </Button>
            </>
          }
        >
          <p className="text-fg-mid">
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
