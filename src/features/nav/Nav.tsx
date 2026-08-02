/**
 * The navigation panel.
 *
 * Resident, around 200px, drag-resizable, folded away by a visible control and
 * never summoned by a keypress (PLAN.md §M2.5, "Settled in phase 1"). Folded,
 * it becomes a 44px icon strip that keeps queue badges on screen.
 *
 * Groups, in order: Library, Pinned, Folders, Saved searches, Queues.
 *
 * Two things the tree must not do, both from docs/DESIGN.md §2:
 *
 * - **The library root is not a node.** Everything, Loose items and Favourites
 *   are their own rows above the tree, never inside it, and an empty tree
 *   renders as empty rather than as a lone root.
 * - **The tree never reorders.** Pinned folders get their own group above it,
 *   so favouriting something never moves the row you reach for.
 */

import { useMemo, useState } from "react";

import { IconButton } from "../../components/Button";
import { ContextMenu } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { formatCount } from "../../lib/format";
import type { ArchetypeInfo, FolderNode, FolderStatusDef } from "../../lib/types";
import type { Scope } from "../../state/library";
import { NAV_FOLDED } from "../../state/ui";
import { FolderMenu, FolderTreeBackgroundMenu } from "../menus/FolderMenu";

/** The one status that gets a mark in the tree. One mark, not four:
 *  docs/DESIGN.md §1 "Folders" — absence means nothing to say. */
const MARKED_STATUS = "wip";

export interface NavProps {
  folders: FolderNode[];
  scope: Scope;
  onScope: (scope: Scope) => void;
  statuses: FolderStatusDef[];
  archetypes: ArchetypeInfo[];
  folded: boolean;
  onFoldedChange: (folded: boolean) => void;
  /** Open the band expanded on this folder — where fields and tags are
   *  edited. */
  onEditDetails: (folder: FolderNode) => void;
  favouriteCount: number;
}

export function Nav(props: NavProps) {
  return props.folded ? <FoldedNav {...props} /> : <ExpandedNav {...props} />;
}

// --- folded ----------------------------------------------------------------

function FoldedNav({
  folders,
  scope,
  onScope,
  onFoldedChange,
  favouriteCount,
}: NavProps) {
  const queues = queueFolders(folders);

  return (
    <nav
      aria-label="Navigation"
      style={{ width: NAV_FOLDED }}
      className="flex shrink-0 flex-col items-center gap-1 border-r border-line bg-panel py-2"
    >
      <Tooltip label="Show the navigation panel">
        <IconButton
          aria-label="Show the navigation panel"
          onClick={() => onFoldedChange(false)}
        >
          »
        </IconButton>
      </Tooltip>

      <div className="my-1 h-px w-6 bg-line-soft" />

      <StripButton
        glyph="▦"
        label="Everything"
        active={scope.kind === "everything"}
        onClick={() => onScope({ kind: "everything", folder: null, recursive: true })}
      />
      <StripButton
        glyph="◇"
        label="Loose items"
        active={scope.kind === "loose"}
        onClick={() => onScope({ kind: "loose", folder: null, recursive: false })}
      />
      <StripButton
        glyph="★"
        label="Favourites"
        badge={favouriteCount}
        active={scope.kind === "favourites"}
        onClick={() => onScope({ kind: "favourites", folder: null, recursive: true })}
      />

      {queues.length > 0 && <div className="my-1 h-px w-6 bg-line-soft" />}

      {/* Folded keeps queue badges on screen — that is the whole point of
          the strip, and why the badge lives on the button rather than in a
          column that disappears with the labels. */}
      {queues.map((folder) => (
        <StripButton
          key={folder.id}
          glyph="⌸"
          label={folder.title}
          badge={folder.totalCount}
          active={scope.folder === folder.relPath}
          onClick={() =>
            onScope({ kind: "folder", folder: folder.relPath, recursive: true })
          }
        />
      ))}
    </nav>
  );
}

function StripButton({
  glyph,
  label,
  badge,
  active,
  onClick,
}: {
  glyph: string;
  label: string;
  badge?: number;
  active?: boolean;
  onClick: () => void;
}) {
  return (
    <Tooltip label={badge ? `${label} — ${formatCount(badge)}` : label}>
      <button
        type="button"
        aria-label={label}
        onClick={onClick}
        className={`relative grid h-[30px] w-[30px] place-items-center rounded-[4px] text-[13px] ${
          active ? "bg-accent/15 text-accent" : "text-fg-mid hover:bg-hover hover:text-fg"
        }`}
      >
        {glyph}
        {badge !== undefined && badge > 0 && (
          <span className="absolute -right-0.5 -top-0.5 rounded-full bg-raised px-1 font-mono text-[9px] leading-[13px] text-fg-mid">
            {formatCount(badge)}
          </span>
        )}
      </button>
    </Tooltip>
  );
}

// --- expanded --------------------------------------------------------------

function ExpandedNav({
  folders,
  scope,
  onScope,
  statuses,
  archetypes,
  onFoldedChange,
  onEditDetails,
  favouriteCount,
}: NavProps) {
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
    // Sorted by title and nothing else. Pinning must never move a row.
    for (const siblings of childrenOf.values()) {
      siblings.sort((a, b) => a.title.localeCompare(b.title));
    }
    return { root, childrenOf };
  }, [folders]);

  const pinned = folders.filter((folder) => folder.parentId !== null && folder.favorite);
  const queues = queueFolders(folders);

  const openFolder = (folder: FolderNode) =>
    onScope({ kind: "folder", folder: folder.relPath, recursive: true });

  const rowFor = (folder: FolderNode, depth: number, key: string) => (
    <FolderRow
      key={key}
      folder={folder}
      depth={depth}
      selected={scope.kind === "folder" && scope.folder === folder.relPath}
      expandable={(childrenOf.get(folder.id) ?? []).length > 0}
      expanded={!collapsed.has(folder.id)}
      statuses={statuses}
      archetypes={archetypes}
      onToggle={() =>
        setCollapsed((current) => {
          const next = new Set(current);
          if (next.has(folder.id)) next.delete(folder.id);
          else next.add(folder.id);
          return next;
        })
      }
      onOpen={openFolder}
      onEditDetails={onEditDetails}
    />
  );

  const queueIds = new Set(queues.map((folder) => folder.id));
  const tree: React.ReactNode[] = [];
  const push = (folder: FolderNode, depth: number) => {
    // A queue has its own group; showing it here as well would put the same
    // row on screen twice.
    if (queueIds.has(folder.id)) return;
    tree.push(rowFor(folder, depth, `tree-${folder.id}`));
    if (collapsed.has(folder.id)) return;
    for (const child of childrenOf.get(folder.id) ?? []) {
      push(child, depth + 1);
    }
  };
  for (const child of root ? (childrenOf.get(root.id) ?? []) : []) {
    push(child, 0);
  }

  return (
    <nav
      aria-label="Navigation"
      className="flex min-h-0 w-full flex-1 flex-col overflow-hidden border-r border-line bg-panel"
    >
      <div className="flex items-center gap-1 px-2 py-1.5">
        <span className="flex-1 truncate pl-1 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
          Library
        </span>
        <Tooltip label="Hide the navigation panel" side="bottom">
          <IconButton
            aria-label="Hide the navigation panel"
            onClick={() => onFoldedChange(true)}
          >
            «
          </IconButton>
        </Tooltip>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pb-3">
        {/* Library — the three navigation roots, above the tree and never
            nodes in it. */}
        <RootRow
          label="Everything"
          glyph="▦"
          active={scope.kind === "everything"}
          onClick={() => onScope({ kind: "everything", folder: null, recursive: true })}
        />
        <RootRow
          label="Loose items"
          glyph="◇"
          active={scope.kind === "loose"}
          onClick={() => onScope({ kind: "loose", folder: null, recursive: false })}
        />
        <RootRow
          label="Favourites"
          glyph="★"
          count={favouriteCount}
          active={scope.kind === "favourites"}
          onClick={() => onScope({ kind: "favourites", folder: null, recursive: true })}
        />

        {pinned.length > 0 && (
          <>
            <GroupLabel>Pinned</GroupLabel>
            {pinned.map((folder) => rowFor(folder, 0, `pinned-${folder.id}`))}
          </>
        )}

        <ContextMenu menu={<FolderTreeBackgroundMenu root={root} />}>
          <div className="min-h-[40px]">
            <GroupLabel>Folders</GroupLabel>
            {tree}
            {tree.length === 0 && (
              // An empty tree renders as empty — not as a root node, not as a
              // placeholder branch. DESIGN.md §2 "Navigation roots".
              <p className="px-3 py-1.5 text-[12px] text-fg-dim">
                No folders yet. Right-click here to make one.
              </p>
            )}
          </div>
        </ContextMenu>

        {/* Saved searches are M3's, and queues arrive with the features that
            fill them — Sorting Box in M4, Pending Review in M6. A group with
            nothing in it is not drawn: an empty heading is a promise, and
            this panel should only ever show what can actually be opened. */}
        {queues.length > 0 && (
          <>
            <GroupLabel>Queues</GroupLabel>
            {queues.map((folder) => rowFor(folder, 0, `queue-${folder.id}`))}
          </>
        )}
      </div>
    </nav>
  );
}

function GroupLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 pb-1 pt-3 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
      {children}
    </div>
  );
}

function RootRow({
  label,
  glyph,
  count,
  active,
  onClick,
}: {
  label: string;
  glyph: string;
  count?: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={`flex w-full items-center gap-2 border-l-2 py-[5px] pl-3 pr-2 text-left ${
        active
          ? "border-l-accent bg-accent/10 text-fg"
          : "border-l-transparent text-fg-mid hover:bg-hover hover:text-fg"
      }`}
    >
      <span className="w-3 shrink-0 text-center text-[11px] text-fg-dim">{glyph}</span>
      <span className="truncate">{label}</span>
      {count !== undefined && count > 0 && (
        <span className="ml-auto pl-2 font-mono text-[11px] tabular-nums text-fg-dim">
          {formatCount(count)}
        </span>
      )}
    </button>
  );
}

function FolderRow({
  folder,
  depth,
  selected,
  expandable,
  expanded,
  statuses,
  archetypes,
  onToggle,
  onOpen,
  onEditDetails,
}: {
  folder: FolderNode;
  depth: number;
  selected: boolean;
  expandable: boolean;
  expanded: boolean;
  statuses: FolderStatusDef[];
  archetypes: ArchetypeInfo[];
  onToggle: () => void;
  onOpen: (folder: FolderNode) => void;
  onEditDetails: (folder: FolderNode) => void;
}) {
  return (
    <ContextMenu
      menu={
        <FolderMenu
          folder={folder}
          statuses={statuses}
          archetypes={archetypes}
          onOpen={onOpen}
          onEditDetails={onEditDetails}
        />
      }
    >
      <div
        className={`flex w-full items-center border-l-2 ${
          selected
            ? "border-l-accent bg-accent/10 text-fg"
            : "border-l-transparent text-fg-mid hover:bg-hover"
        }`}
      >
        <button
          type="button"
          aria-label={expandable ? (expanded ? "Collapse" : "Expand") : undefined}
          onClick={onToggle}
          disabled={!expandable}
          style={{ marginLeft: 8 + depth * 13 }}
          className="w-3 shrink-0 text-center text-[9px] text-fg-dim hover:text-fg disabled:opacity-0"
        >
          {expanded ? "▾" : "▸"}
        </button>

        <button
          type="button"
          onClick={() => onOpen(folder)}
          className="flex min-w-0 flex-1 items-center gap-1.5 py-[5px] pl-1 pr-2 text-left hover:text-fg"
        >
          <span className="truncate">{folder.title}</span>
          {folder.status === MARKED_STATUS && (
            // One dot, meaning "needs more". Nothing is drawn for any other
            // status, so the mark stays glanceable without a legend.
            <span
              title="Work in progress"
              className="h-1.5 w-1.5 shrink-0 rounded-full bg-fg-mid"
            />
          )}
          <span className="ml-auto shrink-0 pl-2 font-mono text-[11px] tabular-nums text-fg-dim">
            {formatCount(folder.totalCount)}
          </span>
        </button>
      </div>
    </ContextMenu>
  );
}

/**
 * Queue folders that actually exist on disk. The Sorting Box is a real
 * directory (PLAN.md decision 6) rather than a virtual view, so it appears
 * here as soon as there is one and behaves like any other folder until M4
 * gives it triage.
 */
function queueFolders(folders: FolderNode[]): FolderNode[] {
  return folders.filter(
    (folder) => folder.parentId !== null && folder.relPath === "sorting box",
  );
}
