import { useMemo, useState } from "react";

import { formatCount } from "../../lib/format";
import type { FolderNode } from "../../lib/types";

interface SidebarProps {
  folders: FolderNode[];
  /** Selected folder rel_path, or null for the whole library. */
  selected: string | null;
  onSelect: (relPath: string | null) => void;
}

export function Sidebar({ folders, selected, onSelect }: SidebarProps) {
  const [collapsed, setCollapsed] = useState<Set<number>>(() => new Set());

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
        <span className="truncate">{isRoot ? "Library" : folder.title}</span>
        <span className="ml-auto pl-2 font-mono text-[11px] tabular-nums text-fg-dim">
          {formatCount(folder.totalCount)}
        </span>
      </button>,
    );

    if (!isOpen) return;
    for (const child of children.sort((a, b) => a.title.localeCompare(b.title))) {
      push(child, depth + 1);
    }
  };

  if (root) push(root, 0);

  return (
    <aside className="w-[214px] shrink-0 overflow-y-auto border-r border-line bg-panel py-2.5">
      <div className="px-3.5 pb-1.5 pt-3 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
        Library
      </div>
      {rows}
      {folders.length === 0 && (
        <div className="px-3.5 py-2 text-fg-dim">No folders indexed yet.</div>
      )}
    </aside>
  );
}
