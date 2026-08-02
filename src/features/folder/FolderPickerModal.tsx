import type { FolderNode } from "../../lib/types";

interface FolderPickerModalProps {
  folders: FolderNode[];
  /** Folder ids that can't be picked — used when moving a folder itself, so
   *  it can't be dropped into its own subtree. Item moves need nothing
   *  excluded. */
  exclude?: Set<number>;
  onPick: (folderId: number) => void;
  onClose: () => void;
}

/**
 * A flat indented folder list, reused for both "move this folder" and "move
 * selected items" — the cheapest destination picker that exercises the
 * move operations. Real drag-and-drop is M2.5's (docs/DESIGN.md "Folder
 * operations").
 */
export function FolderPickerModal({
  folders,
  exclude,
  onPick,
  onClose,
}: FolderPickerModalProps) {
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

  const rows: React.ReactNode[] = [];
  const push = (folder: FolderNode, depth: number) => {
    const disabled = exclude?.has(folder.id) ?? false;
    rows.push(
      <button
        key={folder.id}
        type="button"
        disabled={disabled}
        onClick={() => onPick(folder.id)}
        style={{ paddingLeft: 12 + depth * 14 }}
        className="flex w-full items-center py-1 pr-3 text-left text-fg-mid hover:bg-hover hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
      >
        {folder.parentId === null ? "Library" : folder.title}
      </button>,
    );
    const children = (childrenOf.get(folder.id) ?? []).sort((a, b) =>
      a.title.localeCompare(b.title),
    );
    for (const child of children) push(child, depth + 1);
  };
  if (root) push(root, 0);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[70vh] w-[360px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Move to…</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>
        <div className="min-h-0 flex-1 overflow-y-auto py-1 text-[13px]">
          {rows}
          {folders.length === 0 && (
            <div className="px-3 py-2 text-fg-dim">No folders yet.</div>
          )}
        </div>
      </div>
    </div>
  );
}
