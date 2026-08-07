/**
 * The pane's Folders mode — destination tiles, and the reason the
 * two-Explorer-window workflow this app replaces is beaten rather than tied.
 * SPEC.md §2 *Folders mode*.
 *
 * - **One flat field per level.** No sections, no reordering, no sorting by
 *   recency — a folder is where it was last time, which is what lets the
 *   drag become muscle memory.
 * - **Single click drills in**, staying inside this mode; the main grid does
 *   not move. **Double click** navigates the main grid there.
 * - **Breadcrumb and an Up control**, both drop targets, same as every tile.
 * - **A filter box pinned to the bottom** searches title and path across the
 *   whole library, flat, ignoring wherever this view had drilled to.
 *   Clearing it restores the drilled position.
 * - **A "+ New folder in ‹parent›" tile is always present**, inline rather
 *   than a modal — the actual inline folder creation §4 needs, as a visible
 *   control rather than a keystroke (locked decision 23). While filtering
 *   with nothing matching, a **"Create '‹query›' in ‹parent›" row** appears
 *   instead, creating in the drilled parent regardless of the flat search.
 * - **Dragging a folder onto a tile nests it.** A tile that would become its
 *   own descendant refuses visibly — the backend's own refusal surfaces as
 *   the ordinary failure toast, the same path every other operation here
 *   uses.
 */

import { ArrowUp, Folder, Plus } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";

import { Breadcrumb } from "../../components/Breadcrumb";
import { Button, IconButton } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { Tooltip } from "../../components/Tooltip";
import { ancestorTitles } from "../../lib/folders";
import { formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import { assetUrl } from "../../lib/ipc";
import type { FolderNode } from "../../lib/types";
import { cn } from "../../lib/utils";
import { dropIsValid, onFolderDragStart, resolveDrop, useDnd, useSpringLoad } from "../../state/dnd";
import type { PaneMode } from "../../state/ui";
import { useOperations } from "../menus/operations";
import { PaneFrame } from "./PaneFrame";

export interface FoldersModeProps {
  mode: PaneMode;
  onModeChange: (mode: PaneMode) => void;
  onClose: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;
  folders: FolderNode[];
  refreshToken: number;
  thumbsDir: string;
  /** Double click — the one gesture that moves the main grid. */
  onOpenInMain: (folder: FolderNode) => void;
}

export function FoldersMode({
  mode,
  onModeChange,
  onClose,
  maximised,
  onMaximisedChange,
  folders,
  refreshToken,
  thumbsDir,
  onOpenInMain,
}: FoldersModeProps) {
  const ops = useOperations();
  const [parentId, setParentId] = useState<number | null>(null);
  const [filter, setFilter] = useState("");
  const [creating, setCreating] = useState(false);

  const byId = useMemo(() => new Map(folders.map((node) => [node.id, node])), [folders]);
  const parent = parentId !== null ? (byId.get(parentId) ?? null) : null;
  // The drilled folder can vanish out from under this view — deleted, or
  // moved somewhere this pane never sees the intermediate steps of. Falling
  // back to the top level is the same rule `GridMode` and the main grid's
  // own folder both follow.
  useEffect(() => {
    if (parentId !== null && !parent) setParentId(null);
  }, [parentId, parent]);

  const pathFor = (folder: FolderNode) => ancestorTitles(folders, folder.id).join("/");

  const children = useMemo(
    () =>
      folders
        .filter((node) => node.parentId === parentId)
        .sort((a, b) => a.title.localeCompare(b.title)),
    [folders, parentId],
  );

  const needle = filter.trim().toLowerCase();
  const filtered = useMemo(() => {
    if (!needle) return null;
    return folders
      .filter(
        (node) =>
          node.title.toLowerCase().includes(needle) || pathFor(node).toLowerCase().includes(needle),
      )
      .sort((a, b) => a.title.localeCompare(b.title));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [folders, needle]);

  const shown = filtered ?? children;

  const [covers, setCovers] = useState<Record<number, string | null>>({});
  useEffect(() => {
    let cancelled = false;
    Promise.all(
      shown.map((node) =>
        ipc
          .getFolder(node.id)
          .then((detail) => [node.id, detail.coverThumb] as const)
          .catch(() => [node.id, null] as const),
      ),
    ).then((entries) => {
      if (!cancelled) setCovers(Object.fromEntries(entries));
    });
    return () => {
      cancelled = true;
    };
    // Bounded by whatever is on screen — one level's siblings, or a filtered
    // result set — never the whole tree, unlike fetching a cover per row in
    // `db::folders::tree` itself would be (decision 20).
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [shown, refreshToken]);

  const crumbTitles = parent ? ancestorTitles(folders, parent.id) : [];

  return (
    <PaneFrame
      mode={mode}
      onModeChange={onModeChange}
      maximised={maximised}
      onMaximisedChange={onMaximisedChange}
      onClose={onClose}
      header={
        <div className="flex min-w-0 flex-1 items-center gap-1">
          <UpTarget
            enabled={parent !== null}
            grandparentId={parent ? parent.parentId : null}
            folders={folders}
            onNavigate={setParentId}
          />
          {parent ? (
            <Breadcrumb titles={crumbTitles} />
          ) : (
            <span className="truncate text-fg-mid">All folders</span>
          )}
        </div>
      }
    >
      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <div className="grid grid-cols-[repeat(auto-fill,minmax(120px,1fr))] gap-2">
          {shown.map((node) => (
            <FolderTile
              key={node.id}
              folder={node}
              cover={covers[node.id] ?? null}
              thumbsDir={thumbsDir}
              path={filtered ? pathFor(node) : undefined}
              onDrill={() => setParentId(node.id)}
              onOpenInMain={() => onOpenInMain(node)}
            />
          ))}

          {!filtered && !creating && (
            <NewFolderTile parentTitle={parent ? parent.title : "the top level"} onClick={() => setCreating(true)} />
          )}
          {!filtered && creating && (
            <NewFolderInput
              onCancel={() => setCreating(false)}
              onSubmit={(name) => {
                setCreating(false);
                void ops.createFolder(parentId, parent ? parent.title : "the top level", name, null);
              }}
            />
          )}
        </div>

        {filtered && filtered.length === 0 && (
          <button
            type="button"
            onClick={() =>
              void ops.createFolder(parentId, parent ? parent.title : "the top level", filter.trim(), null)
            }
            disabled={!needle}
            className="flex h-9 w-full items-center gap-2 rounded-[4px] border border-dashed border-line px-2.5 text-left text-fg-mid hover:border-accent-d hover:text-fg"
          >
            <Plus className="size-4 shrink-0" />
            <span className="truncate">
              Create &ldquo;{filter.trim()}&rdquo; in {parent ? parent.title : "the top level"}
            </span>
          </button>
        )}
      </div>

      <div className="flex h-11 shrink-0 items-center border-t border-line px-2">
        <Input
          value={filter}
          onChange={(event) => setFilter(event.target.value)}
          placeholder="Filter By Title Or Path…"
          aria-label="Filter Folders"
        />
      </div>
    </PaneFrame>
  );
}

/** The Up control — a drop target like every tile, filing whatever is
 *  dropped into the current parent's own parent. Disabled (and not a
 *  target) at the top level, where there is nowhere left to go up to.
 *
 *  Not `resolveDrop` — that helper is written for a target that is always a
 *  real folder, and Up's destination can be the top level itself
 *  (`parentId: null`), a valid destination for a folder move but not for an
 *  item move (there is no "unfile to the Sorting Box by dropping"
 *  operation, only `moveFolder`'s null-parent case). */
function UpTarget({
  enabled,
  grandparentId,
  folders,
  onNavigate,
}: {
  enabled: boolean;
  grandparentId: number | null;
  folders: FolderNode[];
  onNavigate: (id: number | null) => void;
}) {
  const ops = useOperations();
  const { dragging } = useDnd();
  const grandparent = grandparentId !== null ? (folders.find((n) => n.id === grandparentId) ?? null) : null;
  const accepting =
    enabled &&
    dragging !== null &&
    (dragging.kind === "folder" ? dragging.folder.id !== grandparentId : grandparent !== null);
  const springLoad = useSpringLoad(() => onNavigate(grandparentId));

  return (
    <Tooltip label="Up A Level" side="bottom">
      <IconButton
        aria-label="Up A Level"
        disabled={!enabled}
        onClick={() => onNavigate(grandparentId)}
        onDragOver={(event) => {
          if (accepting) event.preventDefault();
        }}
        onDragEnter={springLoad.onDragEnter}
        onDragLeave={springLoad.onDragLeave}
        onDrop={(event) => {
          event.preventDefault();
          springLoad.onDrop();
          if (!dragging || !accepting) return;
          if (dragging.kind === "folder") void ops.moveFolder(dragging.folder, grandparent);
          else if (grandparent) void ops.moveItems(dragging.itemIds, grandparent);
        }}
        className={cn(accepting && "outline outline-2 -outline-offset-2 outline-accent")}
      >
        <ArrowUp />
      </IconButton>
    </Tooltip>
  );
}

function FolderTile({
  folder,
  cover,
  thumbsDir,
  path,
  onDrill,
  onOpenInMain,
}: {
  folder: FolderNode;
  cover: string | null;
  thumbsDir: string;
  /** Set only while the list is flat (filtered) — two folders with the same
   *  title are otherwise indistinguishable. */
  path?: string;
  onDrill: () => void;
  onOpenInMain: () => void;
}) {
  const ops = useOperations();
  const { dragging, startDrag } = useDnd();
  const [hovering, setHovering] = useState(false);
  const accepting = dragging !== null && dropIsValid(dragging, folder.id);
  const springLoad = useSpringLoad(onDrill);

  const previewCount =
    accepting && hovering && dragging?.kind === "items"
      ? folder.totalCount + dragging.itemIds.length
      : null;

  return (
    <button
      type="button"
      draggable
      onDragStart={(event) => onFolderDragStart(event, folder, startDrag)}
      onDragOver={(event) => {
        if (accepting) event.preventDefault();
      }}
      onDragEnter={() => {
        setHovering(true);
        springLoad.onDragEnter();
      }}
      onDragLeave={() => {
        setHovering(false);
        springLoad.onDragLeave();
      }}
      onDrop={(event) => {
        event.preventDefault();
        setHovering(false);
        springLoad.onDrop();
        if (dragging && accepting) resolveDrop(dragging, folder, ops);
      }}
      onClick={onDrill}
      onDoubleClick={onOpenInMain}
      className={cn(
        "flex flex-col items-stretch gap-1.5 rounded-[6px] border border-line bg-panel p-2 text-left hover:border-line-soft hover:bg-raised",
        accepting && "outline outline-2 -outline-offset-2 outline-accent",
      )}
    >
      <div className="flex aspect-square items-center justify-center overflow-hidden rounded-[4px] bg-sunk">
        {cover ? (
          <img
            src={assetUrl(thumbsDir, cover)}
            alt=""
            className="size-full object-cover"
            draggable={false}
          />
        ) : (
          <Folder className="size-8 text-fg-dim" />
        )}
      </div>
      <span className="truncate text-fg">{folder.title}</span>
      {path !== undefined && (
        <span className="truncate font-mono text-11 text-fg-dim">{path}</span>
      )}
      <span className="font-mono text-12 tabular-nums text-fg-dim">
        {previewCount !== null ? (
          <>
            {formatCount(folder.totalCount)} → <span className="text-accent">{formatCount(previewCount)}</span>
          </>
        ) : (
          `${formatCount(folder.totalCount)} item${folder.totalCount === 1 ? "" : "s"}`
        )}
      </span>
    </button>
  );
}

function NewFolderTile({ parentTitle, onClick }: { parentTitle: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex aspect-square flex-col items-center justify-center gap-1.5 rounded-[6px] border border-dashed border-line p-2 text-center text-fg-dim hover:border-accent-d hover:text-fg"
    >
      <Plus className="size-6" />
      <span className="text-12 leading-tight">New Folder In {parentTitle}</span>
    </button>
  );
}

function NewFolderInput({
  onCancel,
  onSubmit,
}: {
  onCancel: () => void;
  onSubmit: (name: string) => void;
}) {
  const [name, setName] = useState("");
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => ref.current?.focus(), []);

  // Clicking the Create button both blurs the input (moving focus away) and
  // fires its own click — without this guard both would call `submit`,
  // creating the folder twice. Once `submit` has acted, every later call
  // (from whichever of the two fires second) is a no-op.
  const done = useRef(false);
  const submit = () => {
    if (done.current) return;
    done.current = true;
    const trimmed = name.trim();
    if (trimmed) onSubmit(trimmed);
    else onCancel();
  };

  return (
    <div className="flex aspect-square flex-col items-stretch justify-center gap-1.5 rounded-[6px] border border-accent-d bg-panel p-2">
      <Input
        ref={ref}
        value={name}
        onChange={(event) => setName(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") submit();
          if (event.key === "Escape") onCancel();
        }}
        onBlur={submit}
        placeholder="Folder Name"
      />
      <Button size="sm" onClick={submit}>
        Create
      </Button>
    </div>
  );
}
