/**
 * In-app drag and drop — ROADMAP.md §M2.5b, SPEC.md §2 *Drops*.
 *
 * Since M2.6 a drop is a row update (`folder_id` for an item, `parent_id`
 * for a folder), not a file move — instant, and one journal entry to
 * reverse. That is what lets this be plain HTML5 drag and drop rather than
 * anything heavier: the payload is small enough to carry in memory, in a
 * context, rather than serialised through `dataTransfer`.
 *
 * **WebView2 swallows HTML5 drag and drop unless told not to** — see
 * `docs/NOTES.md`. `lib.rs`'s `build_window` calls
 * `disable_drag_drop_handler()`, which is what makes any of this fire at
 * all on Windows.
 *
 * Three targets, per SPEC.md §*Drops*: folder tiles (`FoldersMode.tsx`),
 * tree rows (`Nav.tsx`), and the pane in Grid mode (`GridMode.tsx`). All
 * three resolve a drop the same way — `resolveDrop`, below — so a folder
 * dropped onto any of them nests the same way an item dropped onto any of
 * them files the same way.
 */

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useRef,
  useState,
  type DragEvent as ReactDragEvent,
} from "react";

import type { FolderNode } from "../lib/types";
import type { Operations } from "../features/menus/operations";

export type DragPayload =
  | { kind: "items"; itemIds: number[] }
  | { kind: "folder"; folder: FolderNode };

interface DndState {
  dragging: DragPayload | null;
  startDrag: (payload: DragPayload) => void;
}

const DndContext = createContext<DndState | null>(null);

/** One drag in flight for the whole window — there is never more than one
 *  pointer, so a single slot is all a drag source ever needs to publish to.
 *  Cleared on `dragend`, which fires on the source whether the drop landed
 *  on a target, missed every target, or was cancelled with Escape — the one
 *  event guaranteed to fire exactly once per drag, unlike `drop`. */
export function DndProvider({ children }: { children: React.ReactNode }) {
  const [dragging, setDragging] = useState<DragPayload | null>(null);

  const startDrag = useCallback((payload: DragPayload) => {
    setDragging(payload);
  }, []);

  useEffect(() => {
    const onDragEnd = () => setDragging(null);
    window.addEventListener("dragend", onDragEnd);
    return () => window.removeEventListener("dragend", onDragEnd);
  }, []);

  return (
    <DndContext.Provider value={{ dragging, startDrag }}>
      {children}
    </DndContext.Provider>
  );
}

export function useDnd(): DndState {
  const value = useContext(DndContext);
  if (!value) throw new Error("useDnd must be used inside <DndProvider>");
  return value;
}

/** A folder cannot accept itself, and an item drag has nothing to check —
 *  the backend already refuses a folder-into-its-own-descendant move
 *  visibly (`fs::relocate::move_folder`), so this only short-circuits the
 *  one case worth stopping before the request ever goes out: dropping a
 *  folder on itself, which the tree and the tile grid both make trivial to
 *  attempt by accident. */
export function dropIsValid(payload: DragPayload, destFolderId: number): boolean {
  if (payload.kind === "folder") return payload.folder.id !== destFolderId;
  return true;
}

/** What every drop target ultimately does — items move into the folder,
 *  a dragged folder nests inside it. One place, so a folder tile, a tree
 *  row and the pane's Grid mode background can never disagree about what a
 *  drop means. */
export function resolveDrop(payload: DragPayload, dest: FolderNode, ops: Operations): void {
  if (payload.kind === "items") {
    void ops.moveItems(payload.itemIds, dest);
  } else if (payload.folder.id !== dest.id) {
    void ops.moveFolder(payload.folder, dest);
  }
}

/** Spring-loading: hovering a drop target while dragging opens it after a
 *  dwell, so a nested destination can be reached without setting it up
 *  first (SPEC.md §*Drops*). Only armed while a drag is actually in
 *  progress — plain mouse hover must never trigger navigation.
 *
 * `dragenter`/`dragleave` fire on every child element a cursor crosses, not
 * just the target's own boundary, which is the classic flicker source for
 * anything keyed off them directly. An enter counter is the standard fix:
 * only a drop to zero means the pointer actually left the target, not just
 * one of its children. */
export function useSpringLoad(onTrigger: () => void, delayMs = 700) {
  const depth = useRef(0);
  const timer = useRef<number | undefined>(undefined);
  const { dragging } = useDnd();

  const clear = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = undefined;
  }, []);

  useEffect(() => clear, [clear]);
  // A drag that ends mid-hover (dropped elsewhere, or Escaped) must not
  // leave a stale timer armed to fire after the drag is long over.
  useEffect(() => {
    if (!dragging) {
      depth.current = 0;
      clear();
    }
  }, [dragging, clear]);

  const onDragEnter = useCallback(() => {
    if (!dragging) return;
    depth.current += 1;
    if (timer.current === undefined) {
      timer.current = window.setTimeout(() => {
        timer.current = undefined;
        onTrigger();
      }, delayMs);
    }
  }, [dragging, onTrigger, delayMs]);

  const onDragLeave = useCallback(() => {
    if (!dragging) return;
    depth.current = Math.max(0, depth.current - 1);
    if (depth.current === 0) clear();
  }, [dragging, clear]);

  const onDrop = useCallback(() => {
    depth.current = 0;
    clear();
  }, [clear]);

  return { onDragEnter, onDragLeave, onDrop };
}

/** Wires a DOM element up as a drag source for a set of selected items —
 *  used by `Tile.tsx`'s imperative pool, which has no JSX of its own to
 *  attach a `draggable` prop to. */
export function startItemDrag(event: DragEvent, itemIds: number[], onStart: (payload: DragPayload) => void) {
  event.dataTransfer?.setData("text/plain", "");
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  onStart({ kind: "items", itemIds });
}

/** The folder-row/folder-tile equivalent, for plain React `draggable`
 *  elements — sets up the same `dataTransfer` state a browser expects from
 *  a drag source before publishing the payload. */
export function onFolderDragStart(
  event: ReactDragEvent,
  folder: FolderNode,
  startDrag: (payload: DragPayload) => void,
) {
  event.dataTransfer.setData("text/plain", "");
  event.dataTransfer.effectAllowed = "move";
  startDrag({ kind: "folder", folder });
}
