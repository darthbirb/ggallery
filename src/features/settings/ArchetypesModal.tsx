import { useEffect, useState } from "react";

import * as ipc from "../../lib/ipc";
import type { ArchetypeInfo } from "../../lib/types";

interface ArchetypesModalProps {
  onClose: () => void;
  /** Folder headers show archetype fields — refetch after any edit. */
  onChanged: () => void;
}

const FIELD_TYPES = ["text", "handle", "url", "date", "number"];

/**
 * Create, rename, delete an archetype; add, reorder, remove its fields.
 * Mandatory now that nothing is seeded (PLAN.md decision 21) — disposable
 * scaffolding per §M2.1, M2.5 designs the real editor.
 */
export function ArchetypesModal({ onClose, onChanged }: ArchetypesModalProps) {
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [newName, setNewName] = useState("");
  const [newFieldKey, setNewFieldKey] = useState("");
  const [newFieldType, setNewFieldType] = useState(FIELD_TYPES[0]);
  const [error, setError] = useState<string | null>(null);

  const refresh = async () => {
    try {
      const list = await ipc.listArchetypes();
      setArchetypes(list);
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

  const selected = archetypes.find((a) => a.id === selectedId) ?? null;

  const createArchetype = () => {
    const name = newName.trim();
    if (!name) return;
    void run(async () => {
      const id = await ipc.createArchetype(name);
      setNewName("");
      setSelectedId(id);
    });
  };

  const removeField = (key: string) => {
    if (!selected) return;
    void (async () => {
      try {
        const usage = await ipc.archetypeFieldUsage(selected.id, key);
        if (usage.length > 0) {
          const names = usage.map((u) => u.title).join(", ");
          if (!confirm(`Remove "${key}"? This deletes its value on: ${names}.`)) return;
        }
        await ipc.removeArchetypeField(selected.id, key);
        await refresh();
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const addField = () => {
    if (!selected) return;
    const key = newFieldKey.trim();
    if (!key) return;
    void (async () => {
      try {
        const count = await ipc.countFoldersUsingArchetype(selected.id);
        const applyToExisting =
          count > 0
            ? confirm(`${count} folder(s) use this archetype — add "${key}" to them?`)
            : false;
        await ipc.addArchetypeField(selected.id, key, newFieldType, applyToExisting);
        setNewFieldKey("");
        await refresh();
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const moveField = (key: string, direction: -1 | 1) => {
    if (!selected) return;
    const keys = selected.fields.map((f) => f.key);
    const index = keys.indexOf(key);
    const swapWith = index + direction;
    if (swapWith < 0 || swapWith >= keys.length) return;
    [keys[index], keys[swapWith]] = [keys[swapWith], keys[index]];
    void run(() => ipc.reorderArchetypeFields(selected.id, keys));
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[82vh] w-[640px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Archetypes</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="flex min-h-0 flex-1 text-[13px]">
          <div className="flex w-[180px] shrink-0 flex-col overflow-y-auto border-r border-line py-2">
            {archetypes.map((a) => (
              <button
                key={a.id}
                type="button"
                onClick={() => setSelectedId(a.id)}
                className={`flex w-full items-center px-3 py-1.5 text-left ${
                  a.id === selectedId
                    ? "bg-hover font-semibold text-fg"
                    : "text-fg-mid hover:bg-hover hover:text-fg"
                }`}
              >
                {a.name}
              </button>
            ))}
            {archetypes.length === 0 && (
              <p className="px-3 py-1.5 text-fg-dim">No archetypes yet.</p>
            )}
            <div className="mt-2 px-2">
              <input
                value={newName}
                onChange={(event) => setNewName(event.target.value)}
                onKeyDown={(event) => event.key === "Enter" && createArchetype()}
                placeholder="+ new archetype"
                className="w-full rounded-[3px] border border-dashed border-line-soft bg-transparent px-1.5 py-0.5 text-fg placeholder:text-fg-dim"
              />
            </div>
          </div>

          <div className="min-h-0 flex-1 overflow-y-auto p-4">
            {error && (
              <p className="mb-3 rounded-[3px] border border-danger/40 bg-raised px-3 py-2 text-danger">
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
                    onBlur={(event) => {
                      const value = event.target.value.trim();
                      if (value && value !== selected.name) {
                        void run(() => ipc.renameArchetype(selected.id, value));
                      }
                    }}
                    className="flex-1 rounded-[3px] border border-line-soft bg-ground px-1.5 py-1 font-semibold text-fg"
                  />
                  <button
                    type="button"
                    onClick={() => {
                      if (confirm(`Delete "${selected.name}"? Folders keep their existing labels.`)) {
                        void run(() => ipc.deleteArchetype(selected.id));
                        setSelectedId(null);
                      }
                    }}
                    className="text-danger hover:opacity-80"
                  >
                    delete archetype
                  </button>
                </div>

                <table className="w-full border-collapse text-left">
                  <thead>
                    <tr className="text-fg-dim">
                      <th className="py-1 pr-2">key</th>
                      <th className="py-1 pr-2">type</th>
                      <th className="py-1"></th>
                    </tr>
                  </thead>
                  <tbody>
                    {selected.fields.map((field, i) => (
                      <tr key={field.key} className="border-t border-line-soft/60">
                        <td className="py-1 pr-2 font-mono">{field.key}</td>
                        <td className="py-1 pr-2 text-fg-dim">{field.type}</td>
                        <td className="py-1 text-right">
                          <button
                            type="button"
                            disabled={i === 0}
                            onClick={() => moveField(field.key, -1)}
                            className="px-1 text-fg-dim hover:text-fg disabled:opacity-30"
                          >
                            ↑
                          </button>
                          <button
                            type="button"
                            disabled={i === selected.fields.length - 1}
                            onClick={() => moveField(field.key, 1)}
                            className="px-1 text-fg-dim hover:text-fg disabled:opacity-30"
                          >
                            ↓
                          </button>
                          <button
                            type="button"
                            onClick={() => removeField(field.key)}
                            className="px-1 text-fg-dim hover:text-danger"
                          >
                            ×
                          </button>
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
                    onChange={(event) => setNewFieldKey(event.target.value)}
                    onKeyDown={(event) => event.key === "Enter" && addField()}
                    placeholder="field key"
                    className="flex-1 rounded-[3px] border border-line-soft bg-ground px-1.5 py-0.5 text-fg"
                  />
                  <select
                    value={newFieldType}
                    onChange={(event) => setNewFieldType(event.target.value)}
                    className="rounded-[3px] border border-line-soft bg-ground px-1 py-0.5 text-fg-mid"
                  >
                    {FIELD_TYPES.map((t) => (
                      <option key={t} value={t}>
                        {t}
                      </option>
                    ))}
                  </select>
                  <button
                    type="button"
                    onClick={addField}
                    className="rounded-[3px] border border-line px-2 py-0.5 text-fg-mid hover:bg-hover"
                  >
                    add field
                  </button>
                </div>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
