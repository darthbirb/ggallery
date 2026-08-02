import { useMemo, useState } from "react";

import { formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { FolderNode } from "../../lib/types";

/** Mirrors `002_folder_metadata.sql`'s seed. The sidebar renders a plain dot
 *  from the status key rather than round-tripping through IPC for four
 *  colours that don't change at runtime — the folder header is the source of
 *  truth for anything that does. */
const STATUS_COLOURS: Record<string, string> = {
  active: "#6b7280",
  wip: "#eab308",
  done: "#22c55e",
  archived: "#64748b",
};

interface SidebarProps {
  folders: FolderNode[];
  /** Selected folder rel_path, or null for the whole library. */
  selected: string | null;
  onSelect: (relPath: string | null) => void;
  /** The tree needs refetching once a folder is created. */
  onChanged: () => void;
}

export function Sidebar({ folders, selected, onSelect, onChanged }: SidebarProps) {
  const [collapsed, setCollapsed] = useState<Set<number>>(() => new Set());
  const [creating, setCreating] = useState(false);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);

  const { root, childrenOf } = useMemo(() => {
    const childrenOf = new Map<number, FolderNode[]>();
    let root: FolderNode | null = null;
    for (const folder of folders) {
      if (folder.parentId === null) {
        root = folder;
        continue;
      }
      const siblings = childrenOf.get(folder.parentId) ?? [];
      siblings.push(folder);
      childrenOf.set(folder.parentId, siblings);
    }
    return { root, childrenOf };
  }, [folders]);

  const toggle = (id: number) => {
    setCollapsed((current) => {
      const next = new Set(current);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const rows: React.ReactNode[] = [];
  const push = (folder: FolderNode, depth: number) => {
    const children = childrenOf.get(folder.id) ?? [];
    const isOpen = !collapsed.has(folder.id);
    const isRoot = folder.parentId === null;

    rows.push(
      <button
        key={folder.id}
        type="button"
        onClick={() => onSelect(isRoot ? null : folder.relPath)}
        className={`flex w-full items-center gap-1.5 border-l-2 py-[5px] pr-3 text-left ${
          (isRoot && selected === null) || selected === folder.relPath
            ? "border-l-accent bg-hover font-semibold text-fg"
            : "border-l-transparent text-fg-mid hover:bg-hover hover:text-fg"
        }`}
        style={{ paddingLeft: 12 + depth * 13 }}
      >
        <span
          className="w-3 shrink-0 text-center text-[9px] text-fg-dim"
          onClick={(event) => {
            if (children.length === 0) return;
            event.stopPropagation();
            toggle(folder.id);
          }}
        >
          {children.length > 0 ? (isOpen ? "▾" : "▸") : ""}
        </span>
        {!isRoot && (
          <span
            className="h-1.5 w-1.5 shrink-0 rounded-full"
            style={{ backgroundColor: STATUS_COLOURS[folder.status] ?? "#888" }}
            title={folder.status}
          />
        )}
        <span className="truncate">
          {isRoot ? "Library" : folder.title}
          {!isRoot && folder.favorite && " ★"}
        </span>
        <span className="ml-auto pl-2 font-mono text-[11px] tabular-nums text-fg-dim">
          {formatCount(folder.totalCount)}
        </span>
      </button>,
    );

    if (!isOpen) return;
    const ordered = children.sort(
      (a, b) => Number(b.favorite) - Number(a.favorite) || a.title.localeCompare(b.title),
    );
    for (const child of ordered) {
      push(child, depth + 1);
    }
  };

  if (root) push(root, 0);

  const parentId = selected
    ? (folders.find((f) => f.relPath === selected)?.id ?? null)
    : (root?.id ?? null);

  const submit = async () => {
    const trimmed = name.trim();
    if (!trimmed) {
      setCreating(false);
      return;
    }
    try {
      await ipc.createFolder(parentId, trimmed, null);
      setCreating(false);
      setName("");
      setError(null);
      onChanged();
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  };

  return (
    <aside className="w-[214px] shrink-0 overflow-y-auto border-r border-line bg-panel py-2.5">
      <div className="flex items-center px-3.5 pb-1.5 pt-3 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
        Library
        <button
          type="button"
          title="New folder"
          onClick={() => {
            setCreating(true);
            setError(null);
          }}
          className="ml-auto text-[13px] normal-case tracking-normal text-fg-dim hover:text-fg"
        >
          +
        </button>
      </div>
      {creating && (
        <div className="px-3.5 pb-1.5">
          <input
            autoFocus
            value={name}
            onChange={(event) => setName(event.target.value)}
            onBlur={submit}
            onKeyDown={(event) => {
              if (event.key === "Enter") event.currentTarget.blur();
              if (event.key === "Escape") {
                setCreating(false);
                setName("");
              }
            }}
            placeholder="Folder name"
            className="w-full rounded-[3px] border border-accent-d bg-ground px-1.5 py-0.5 text-fg"
          />
        </div>
      )}
      {error && (
        <div className="px-3.5 pb-1.5 text-danger">{error}</div>
      )}
      {rows}
      {folders.length === 0 && (
        <div className="px-3.5 py-2 text-fg-dim">No folders indexed yet.</div>
      )}
    </aside>
  );
}
