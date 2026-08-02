import { useCallback, useEffect, useState } from "react";

import { formatCount, formatTimeAgo } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { ArchetypeInfo, FolderDetail, FolderNode, FolderStatusDef } from "../../lib/types";
import { FolderPickerModal } from "./FolderPickerModal";

interface FolderHeaderProps {
  folderId: number;
  collapsed: boolean;
  onToggleCollapsed: () => void;
  /** Title, status and favorite all affect the sidebar tree — this asks the
   *  parent to re-fetch it after an edit lands. */
  onChanged: () => void;
  /** The whole tree — needed for the "move to…" picker and to compute which
   *  folders are this one's descendants (can't move into itself). */
  folders: FolderNode[];
  /** Called after this folder is deleted, so the parent can navigate away
   *  from a view that no longer exists. */
  onDeleted: () => void;
}

/**
 * Cover, title, archetype fields, flags, status, favorite, notes, counts.
 * Enough UI to exercise M2's data model — the visual pass is M2.5, per
 * PLAN.md §M2, so this deliberately stays plain elements and Tailwind
 * utilities, no component library.
 */
export function FolderHeader({
  folderId,
  collapsed,
  onToggleCollapsed,
  onChanged,
  folders,
  onDeleted,
}: FolderHeaderProps) {
  const [detail, setDetail] = useState<FolderDetail | null>(null);
  const [archetypes, setArchetypes] = useState<ArchetypeInfo[]>([]);
  const [statuses, setStatuses] = useState<FolderStatusDef[]>([]);
  const [showMove, setShowMove] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDetail(await ipc.getFolder(folderId));
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, [folderId]);

  useEffect(() => {
    setDetail(null);
    void refresh();
  }, [refresh]);

  useEffect(() => {
    (async () => {
      try {
        const [a, s] = await Promise.all([
          ipc.listArchetypes(),
          ipc.listFolderStatuses(),
        ]);
        setArchetypes(a);
        setStatuses(s);
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  }, []);

  const commit = useCallback(
    async (action: () => Promise<void>) => {
      try {
        await action();
        await refresh();
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    },
    [refresh, onChanged],
  );

  if (!detail) {
    return (
      <div className="border-b border-line bg-panel px-4 py-3 text-fg-dim">
        {error ?? "Loading folder…"}
      </div>
    );
  }

  const status = statuses.find((s) => s.key === detail.status);

  if (collapsed) {
    return (
      <div className="flex items-center gap-2 border-b border-line bg-panel px-4 py-1.5">
        <span
          className="h-2 w-2 shrink-0 rounded-full"
          style={{ backgroundColor: status?.colour ?? "#888" }}
        />
        <span className="text-[13px] font-semibold text-fg">{detail.title}</span>
        <button
          type="button"
          onClick={onToggleCollapsed}
          className="ml-auto text-fg-dim hover:text-fg"
        >
          expand
        </button>
      </div>
    );
  }

  return (
    <div className="border-b border-line bg-panel px-4 py-3 text-[13px]">
      {error && (
        <p className="mb-2 rounded-[3px] border border-danger/40 bg-raised px-2 py-1 text-danger">
          {error}
        </p>
      )}

      <div className="flex items-start gap-3">
        <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-[4px] border border-line-soft bg-raised text-fg-dim">
          ▣
        </div>

        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <EditableText
              value={detail.title}
              className="text-[15px] font-semibold text-fg"
              onCommit={(title) =>
                title.trim() &&
                title !== detail.title &&
                commit(() => ipc.setFolderTitle(folderId, title.trim()))
              }
            />

            {detail.archetypeName ? (
              <span className="rounded-[3px] border border-line-soft px-1.5 py-0.5 text-[11px] uppercase tracking-wide text-fg-dim">
                {detail.archetypeName}
              </span>
            ) : (
              archetypes.length > 0 && (
                <select
                  value=""
                  onChange={(event) => {
                    const id = Number(event.target.value);
                    if (id) commit(() => ipc.applyFolderArchetype(folderId, id));
                  }}
                  className="rounded-[3px] border border-line-soft bg-ground px-1 py-0.5 text-[11px] text-fg-dim"
                >
                  <option value="">+ archetype…</option>
                  {archetypes.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              )
            )}

            <span
              className="ml-auto h-2 w-2 shrink-0 rounded-full"
              style={{ backgroundColor: status?.colour ?? "#888" }}
            />
            <select
              value={detail.status}
              onChange={(event) =>
                commit(() => ipc.setFolderStatus(folderId, event.target.value))
              }
              className="rounded-[3px] border border-line-soft bg-ground px-1 py-0.5 text-[12px] text-fg-mid"
            >
              {statuses.map((s) => (
                <option key={s.key} value={s.key}>
                  {s.label}
                </option>
              ))}
            </select>

            <button
              type="button"
              title={detail.favorite ? "Remove from favorites" : "Add to favorites"}
              onClick={() => commit(() => ipc.setFolderFavorite(folderId, !detail.favorite))}
              className={detail.favorite ? "text-accent" : "text-fg-dim hover:text-fg"}
            >
              {detail.favorite ? "★" : "☆"}
            </button>

            {detail.parentId !== null && (
              <>
                <button
                  type="button"
                  title="Rename the directory on disk"
                  onClick={() => {
                    const name = prompt("Rename folder on disk to:", detail.title);
                    if (name && name.trim()) {
                      commit(() => ipc.renameFolderDir(folderId, name.trim()));
                    }
                  }}
                  className="text-fg-dim hover:text-fg"
                >
                  rename
                </button>
                <button
                  type="button"
                  onClick={() => setShowMove(true)}
                  className="text-fg-dim hover:text-fg"
                >
                  move…
                </button>
                <button
                  type="button"
                  onClick={() => {
                    if (confirm(`Delete "${detail.title}"? It'll move to .gallery/trash.`)) {
                      (async () => {
                        try {
                          await ipc.deleteFolder(folderId);
                          onDeleted();
                        } catch (err) {
                          setError(ipc.errorMessage(err));
                        }
                      })();
                    }
                  }}
                  className="text-danger hover:opacity-80"
                >
                  delete
                </button>
              </>
            )}

            <button
              type="button"
              onClick={onToggleCollapsed}
              className="text-fg-dim hover:text-fg"
            >
              collapse
            </button>
          </div>

          {detail.fields.length > 0 && (
            <div className="mt-1.5 flex flex-col gap-0.5">
              {detail.fields.map((field) => (
                <div key={field.key} className="flex items-center gap-2">
                  <span className="w-20 shrink-0 text-fg-dim">{field.key}</span>
                  <EditableText
                    value={field.value}
                    placeholder="—"
                    className="text-fg-mid"
                    onCommit={(value) =>
                      value !== field.value &&
                      commit(() => ipc.setFolderLabel(folderId, field.key, value))
                    }
                  />
                </div>
              ))}
            </div>
          )}

          <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
            {detail.flags.map((flag) => (
              <span
                key={flag.tagId}
                className="flex items-center gap-1 rounded-full border border-line-soft bg-raised px-2 py-0.5 text-[12px] text-fg-mid"
              >
                {flag.value}
                <button
                  type="button"
                  onClick={() => commit(() => ipc.removeFolderTag(folderId, flag.tagId))}
                  className="text-fg-dim hover:text-danger"
                >
                  ×
                </button>
              </span>
            ))}
            <AddFlag onAdd={(value) => commit(() => ipc.addFolderFlag(folderId, value))} />
          </div>

          <div className="mt-1.5 flex items-center gap-3 font-mono text-[11px] tabular-nums text-fg-dim">
            <span>{formatCount(detail.directCount)} items</span>
            <span>{formatCount(detail.totalCount)} total</span>
            <span>{formatCount(detail.subfolderCount)} subfolders</span>
            {detail.lastAddedAt !== null && (
              <span>last added: {formatTimeAgo(detail.lastAddedAt)}</span>
            )}
          </div>

          <textarea
            defaultValue={detail.notes ?? ""}
            key={detail.id + ":" + (detail.notes ?? "")}
            placeholder="Notes…"
            rows={2}
            onBlur={(event) => {
              const value = event.target.value;
              if (value !== (detail.notes ?? "")) {
                commit(() => ipc.setFolderNotes(folderId, value || null));
              }
            }}
            className="mt-2 w-full resize-none rounded-[3px] border border-line-soft bg-ground px-2 py-1 text-fg-mid placeholder:text-fg-dim"
          />
        </div>
      </div>

      {showMove && (
        <FolderPickerModal
          folders={folders}
          exclude={descendantIds(folders, detail.id, detail.relPath)}
          onClose={() => setShowMove(false)}
          onPick={(destId) => {
            setShowMove(false);
            commit(() => ipc.moveFolder(folderId, destId));
          }}
        />
      )}
    </div>
  );
}

/** This folder and every descendant, by `relPath` prefix — what a folder
 *  can't be moved into without moving it into itself. */
function descendantIds(folders: FolderNode[], id: number, relPath: string): Set<number> {
  const ids = new Set<number>([id]);
  for (const folder of folders) {
    if (folder.relPath === relPath || folder.relPath.startsWith(`${relPath}/`)) {
      ids.add(folder.id);
    }
  }
  return ids;
}

function EditableText({
  value,
  placeholder,
  className,
  onCommit,
}: {
  value: string;
  placeholder?: string;
  className?: string;
  onCommit: (value: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (!editing) {
    return (
      <button
        type="button"
        onClick={() => {
          setDraft(value);
          setEditing(true);
        }}
        className={`text-left hover:underline ${className ?? ""}`}
      >
        {value || <span className="text-fg-dim">{placeholder ?? "click to edit"}</span>}
      </button>
    );
  }

  const finish = () => {
    setEditing(false);
    onCommit(draft);
  };

  return (
    <input
      autoFocus
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={finish}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setDraft(value);
          setEditing(false);
        }
      }}
      className={`rounded-[3px] border border-accent-d bg-ground px-1 ${className ?? ""}`}
    />
  );
}

function AddFlag({ onAdd }: { onAdd: (value: string) => void }) {
  const [value, setValue] = useState("");
  return (
    <input
      value={value}
      onChange={(event) => setValue(event.target.value)}
      onKeyDown={(event) => {
        if (event.key === "Enter" && value.trim()) {
          onAdd(value.trim());
          setValue("");
        }
      }}
      placeholder="+ tag"
      className="w-16 rounded-full border border-dashed border-line-soft bg-transparent px-2 py-0.5 text-[12px] text-fg-mid placeholder:text-fg-dim focus:w-24"
    />
  );
}
