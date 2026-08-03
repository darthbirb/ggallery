/**
 * The navigation panel.
 *
 * Resident, around 200px, drag-resizable, folded away by a visible control and
 * never summoned by a keypress (PLAN.md §M2.5, "Settled in phase 1"). Folded,
 * it becomes a 44px icon strip that keeps badges on screen.
 *
 * Groups, in order: Library, Pinned, Folders, Saved searches, Queues.
 *
 * Three things the panel must not do:
 *
 * - **The library root is not a node.** Everything, the Sorting Box and
 *   Favourites are their own rows above the tree, never inside it, and an
 *   empty tree renders as empty rather than as a lone root (DESIGN.md §2).
 * - **The tree never reorders.** Pinned folders get their own group above it,
 *   so favouriting something never moves the row you reach for.
 * - **There is no `Sorting Box/` directory.** The library root *is* the
 *   Sorting Box — anything sitting loose at the top level is unfiled by
 *   definition (DESIGN.md §2 and §4). A real folder of that name would be a
 *   second way of saying the same thing, so nothing here looks for one.
 *
 * The fold control was M2.5a's worst offender against decision 25 — a bare
 * chevron with no surface, which did not read as a button at all. It is now
 * an ordinary 32×32 icon button with a background, a border and an 18px
 * glyph, like every other control in the app.
 */

import {
  ChevronRight,
  Folder,
  Inbox,
  LayoutGrid,
  PanelLeftClose,
  PanelLeftOpen,
  Star,
  type LucideIcon,
} from "lucide-react";
import { useMemo, useState } from "react";

import { ContextMenu } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { Badge } from "../../components/ui/badge";
import { IconButton } from "../../components/ui/button";
import { formatCount } from "../../lib/format";
import type { ArchetypeInfo, FolderNode, FolderStatusDef } from "../../lib/types";
import { cn } from "../../lib/utils";
import type { Scope } from "../../state/library";
import { FolderMenu, FolderTreeBackgroundMenu } from "../menus/FolderMenu";

/** The one status that gets a mark in the tree. One mark, not four:
 *  docs/DESIGN.md §1 "Folders" — absence means nothing to say. */
const MARKED_STATUS = "wip";

/** The three navigation roots, in the order DESIGN.md §2 fixes them:
 *  Everything, Sorting Box, Favourites, then the tree. */
interface Root {
  key: string;
  label: string;
  icon: LucideIcon;
  scope: Scope;
}

const ROOTS: Root[] = [
  {
    key: "everything",
    label: "Everything",
    icon: LayoutGrid,
    scope: { kind: "everything", folder: null, recursive: true },
  },
  {
    key: "sorting",
    label: "Sorting Box",
    icon: Inbox,
    scope: { kind: "sorting", folder: null, recursive: false },
  },
  {
    key: "favourites",
    label: "Favourites",
    icon: Star,
    scope: { kind: "favourites", folder: null, recursive: true },
  },
];

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
  /** How many items are loose at the top level — the Sorting Box's badge, and
   *  the number that says whether there is triage to do. */
  sortingCount: number;
}

export function Nav(props: NavProps) {
  return props.folded ? <FoldedNav {...props} /> : <ExpandedNav {...props} />;
}

/** The count a root shows, if any. */
function countFor(root: Root, favouriteCount: number, sortingCount: number) {
  if (root.key === "favourites") return favouriteCount;
  if (root.key === "sorting") return sortingCount;
  return undefined;
}

// --- folded ----------------------------------------------------------------

function FoldedNav({
  scope,
  onScope,
  onFoldedChange,
  favouriteCount,
  sortingCount,
}: NavProps) {
  return (
    <nav
      aria-label="Navigation"
      // Width comes from the wrapper in App.tsx, which tweens between this
      // and the expanded width on fold — see the comment there. Filling
      // whatever it is given, rather than sizing itself, is what lets that
      // transition read as one panel narrowing instead of two panels
      // swapping. `fade-in` softens the content swap underneath it.
      className="fade-in flex h-full w-full shrink-0 flex-col border-r border-line bg-panel"
    >
      {/* Folding must read as the panel getting narrower, not as a different
          panel appearing. So the vertical rhythm is identical to `ExpandedNav`
          below and must stay that way: a 44px header with the fold control,
          the same hairline under it, `pt-2`, then 32px rows on an 8px gap —
          the same 8px both above the first row and between every pair after
          it, rather than a smaller gap above than below. Every root ends up
          on the same baseline in both states. */}
      <div className="flex h-11 shrink-0 items-center justify-center border-b border-line-soft">
        <Tooltip label="Show the navigation panel">
          <IconButton
            aria-label="Show the navigation panel"
            onClick={() => onFoldedChange(false)}
          >
            <PanelLeftOpen />
          </IconButton>
        </Tooltip>
      </div>

      <div className="flex flex-col items-center gap-2 pt-2">
        {ROOTS.map((root) => {
          const count = countFor(root, favouriteCount, sortingCount);
          const Icon = root.icon;
          return (
            <Tooltip
              key={root.key}
              label={
                count === undefined ? root.label : `${root.label} — ${formatCount(count)}`
              }
            >
              <IconButton
                aria-label={root.label}
                active={scope.kind === root.scope.kind}
                onClick={() => onScope(root.scope)}
                className="relative"
              >
                <Icon />
                {/* Folded keeps counts on screen — that is the whole point of
                    the strip, and why the badge sits on the button rather than
                    in a column that disappears with the labels. It overhangs
                    by 4px into the 8px gap, so it never reaches the button
                    above however many digits it grows to. */}
                {count !== undefined && count > 0 && (
                  <Badge className="absolute -right-1 -top-1 h-[18px] min-w-[18px] border-line bg-panel px-1.5">
                    {formatCount(count)}
                  </Badge>
                )}
              </IconButton>
            </Tooltip>
          );
        })}
      </div>
    </nav>
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
  sortingCount,
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

  // Nested rather than a flat pushed array, so a folder's children live in
  // one wrapping element that can animate open and shut — decision 27. They
  // stay mounted while collapsed; `inert` (not `display: none`) is what
  // keeps a collapsed subtree out of tab order and off screen readers
  // without losing the ability to animate its reveal.
  const renderNode = (folder: FolderNode, depth: number): React.ReactNode => {
    const children = childrenOf.get(folder.id) ?? [];
    const expandable = children.length > 0;
    const expanded = !collapsed.has(folder.id);
    return (
      <div key={`tree-${folder.id}`}>
        {rowFor(folder, depth, `tree-row-${folder.id}`)}
        {expandable && (
          <div
            inert={!expanded}
            className={cn(
              "grid transition-[grid-template-rows] duration-[180ms] ease-out",
              expanded ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
            )}
          >
            <div className="overflow-hidden">
              {children.map((child) => renderNode(child, depth + 1))}
            </div>
          </div>
        )}
      </div>
    );
  };
  const tree = root ? (childrenOf.get(root.id) ?? []).map((child) => renderNode(child, 0)) : [];

  return (
    <nav
      aria-label="Navigation"
      className="fade-in flex min-h-0 w-full flex-1 flex-col overflow-hidden border-r border-line bg-panel"
    >
      {/* 44px, matching the folded strip's header exactly — see `FoldedNav`.
          Folding must look like the panel narrowing, not like a second panel
          taking over, so every measurement down to the first root row is
          shared between the two. */}
      <div className="flex h-11 shrink-0 items-center gap-1 border-b border-line-soft px-2">
        <span className="min-w-0 flex-1 truncate pl-1 font-mono uppercase tracking-[0.1em] text-fg-dim">
          Library
        </span>
        <Tooltip label="Hide the navigation panel" side="bottom">
          <IconButton
            aria-label="Hide the navigation panel"
            onClick={() => onFoldedChange(true)}
          >
            <PanelLeftClose />
          </IconButton>
        </Tooltip>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto pb-3 pt-2">
        {/* The three navigation roots, above the tree and never nodes in it.
            The 8px gap is the folded strip's gap; the tree below keeps its
            own tighter rhythm, which is a tree's and not a root list's. */}
        <div className="flex flex-col gap-2">
          {ROOTS.map((root) => (
            <RootRow
              key={root.key}
              label={root.label}
              icon={root.icon}
              count={countFor(root, favouriteCount, sortingCount)}
              active={scope.kind === root.scope.kind}
              onClick={() => onScope(root.scope)}
            />
          ))}
        </div>

        {pinned.length > 0 && (
          <>
            <GroupLabel>Pinned</GroupLabel>
            {pinned.map((folder) => rowFor(folder, 0, `pinned-${folder.id}`))}
          </>
        )}

        <ContextMenu menu={<FolderTreeBackgroundMenu root={root} />}>
          <div className="min-h-[44px]">
            <GroupLabel>Folders</GroupLabel>
            {tree}
            {tree.length === 0 && (
              // An empty tree renders as empty — not as a root node, not as a
              // placeholder branch. DESIGN.md §2 "Navigation roots".
              <p className="px-3 py-1.5 text-[13px] text-fg-dim">
                No folders yet. Right-click here to make one.
              </p>
            )}
          </div>
        </ContextMenu>

        {/* Saved searches are M3's; Pending Review and Trash arrive with the
            features that fill them (M6, M4). A group with nothing in it is not
            drawn: an empty heading is a promise, and this panel should only
            ever show what can actually be opened. The Sorting Box is not among
            them — it is a library root now, not a queue folder. */}
      </div>
    </nav>
  );
}

function GroupLabel({ children }: { children: React.ReactNode }) {
  return (
    <div className="px-3 pb-1 pt-4 font-mono uppercase tracking-[0.1em] text-fg-dim">
      {children}
    </div>
  );
}

/** A row in the panel — a root or a folder. 32px tall. Selection is a filled
 *  rounded surface, not a border — the revision to decision 26: a border
 *  suits a tile, where media fills the frame, but a row has no frame and a
 *  border round text reads as a box rather than a state. Accent-tinted
 *  background and accent text mark the selected row; hover on an idle row
 *  gets the same rounded shape in plain neutral, one step lighter than the
 *  panel, and hovering the selected row keeps its accent tint rather than
 *  falling back to that neutral — the two must never read the same. */
// The ring is inset, because a row runs edge to edge and an outside one
// would be clipped by the panel. Everything else about focus is the single
// `:focus-visible` rule in `styles/index.css`.
//
// `mx-2` is what gives the pill a margin off the panel's edges — the same
// gap the pane header leaves around the details toggle, so the two rounded
// controls read as the same kind of shape rather than one floating free and
// one welded to its container. `pl-[5px]` is not arbitrary on top of that:
// 8px of margin plus 5px of padding puts an 18px icon at 13–31, which is
// exactly where the folded strip's 32px button centres it in a 44px column.
// The icons do not move when the panel folds.
const ROW =
  "flex h-8 w-[calc(100%-16px)] mx-2 items-center gap-2 rounded-[4px] pl-[5px] pr-2 text-left focus-visible:-outline-offset-2";

const ROW_ACTIVE = "bg-accent/15 text-accent hover:bg-accent/25";
const ROW_IDLE = "text-fg-mid hover:bg-hover hover:text-fg";

/** No tooltip: the label is right there. The folded strip has one because it
 *  is icon-only, which is the whole reason a tooltip exists. */
function RootRow({
  label,
  icon: Icon,
  count,
  active,
  onClick,
}: {
  label: string;
  icon: LucideIcon;
  count?: number;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={cn(ROW, active ? ROW_ACTIVE : ROW_IDLE)}
    >
      <Icon className={cn("size-[18px] shrink-0", active ? "text-accent" : "text-fg-dim")} />
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {count !== undefined && count > 0 && (
        <Badge variant={active ? "accent" : "default"}>{formatCount(count)}</Badge>
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
      {/* `mb-0.5` is the tree's own tighter rhythm — a visible gap between a
          folder and its subfolders, and between siblings, without stretching
          to the roots list's `gap-2`. */}
      <div className={cn(ROW, "mb-0.5 pl-0 pr-0", selected ? ROW_ACTIVE : ROW_IDLE)}>
        <button
          type="button"
          aria-label={expandable ? (expanded ? "Collapse" : "Expand") : undefined}
          onClick={onToggle}
          disabled={!expandable}
          style={{ marginLeft: 6 + depth * 14 }}
          className="grid size-5 shrink-0 place-items-center rounded-[3px] text-fg-dim hover:bg-hover hover:text-fg disabled:pointer-events-none disabled:opacity-0"
        >
          {/* A single icon that rotates rather than swapping — decision 27:
              transform, not a conditional pair. */}
          <ChevronRight
            className={cn(
              "size-4 transition-transform duration-[120ms] ease-out",
              expanded && "rotate-90",
            )}
          />
        </button>

        <button
          type="button"
          onClick={() => onOpen(folder)}
          className="flex h-full min-w-0 flex-1 items-center gap-2 pr-2 text-left focus-visible:-outline-offset-2"
        >
          <Folder
            className={cn("size-4 shrink-0", selected ? "text-accent" : "text-fg-dim")}
          />
          <span className="min-w-0 truncate">{folder.title}</span>
          {folder.status === MARKED_STATUS && (
            // One dot, meaning "needs more". Nothing is drawn for any other
            // status, so the mark stays glanceable without a legend.
            <span
              title="Work in progress"
              className="size-1.5 shrink-0 rounded-full bg-fg-mid"
            />
          )}
          <span className="ml-auto shrink-0 pl-2 font-mono tabular-nums text-fg-dim">
            {formatCount(folder.totalCount)}
          </span>
        </button>
      </div>
    </ContextMenu>
  );
}
