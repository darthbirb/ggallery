import { useCallback, useEffect, useRef, useState } from "react";

import { PointMenu } from "../../components/Menu";
import type { GridItem } from "../../lib/types";
import type { SelectionController } from "../../state/selection";
import type { LayoutResult } from "./layoutWorker";
import { Scrubber, type ScrubberHandle } from "./Scrubber";
import { TilePool } from "./Tile";
import { rowAt, useJustifiedLayout } from "./useJustifiedLayout";

/** 8px around the grid, 4px between tiles. */
const PADDING = 8;
const GAP = 4;
/** Rows are cheap; a screen of overscan either side hides decode latency. */
const OVERSCAN = 600;

/** Where the menu was asked for, and on what. `itemId` null is the
 *  background — the empty-space menu. */
export interface GridMenuTarget {
  x: number;
  y: number;
  itemId: number | null;
}

interface GridProps {
  items: GridItem[];
  thumbsDir: string;
  spritesDir: string;
  tileHeight: number;
  selection: SelectionController;
  /** Bumped while indexing so tiles whose thumbnail did not exist yet retry. */
  refreshToken: number;
  /** Double-click. Shows the item in the pane. */
  onActivate: (itemId: number) => void;
  /** Builds the menu for whatever was right-clicked. Returning the content
   *  rather than owning the menu keeps the grid ignorant of what an item
   *  operation is. */
  renderMenu: (target: GridMenuTarget) => React.ReactNode;
  empty?: React.ReactNode;
}

export function Grid({
  items,
  thumbsDir,
  spritesDir,
  tileHeight,
  selection,
  refreshToken,
  onActivate,
  renderMenu,
  empty,
}: GridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const poolRef = useRef<TilePool | null>(null);
  const scrubberRef = useRef<ScrubberHandle>(null);
  const layoutRef = useRef<LayoutResult | null>(null);
  const frameRef = useRef(0);

  const [menu, setMenu] = useState<GridMenuTarget | null>(null);

  // `selection.click` changes identity whenever `items` or the shift-click
  // anchor changes — e.g. on every reload while indexing. The tile pool must
  // still be built exactly once (ENGINEERING-NOTES.md §1: recreating it on
  // every items reload would reintroduce the GC-churn fling regression), so
  // the pool's stable callbacks read through refs to whatever the latest
  // handlers are, rather than closing over them directly.
  const handlers = useRef({ click: selection.click, activate: onActivate });
  handlers.current = { click: selection.click, activate: onActivate };

  const [width, setWidth] = useState(0);
  const layout = useJustifiedLayout(
    items,
    Math.max(width - PADDING * 2, 0),
    tileHeight,
    GAP,
  );
  layoutRef.current = layout;

  // One repaint per animation frame, whatever fires — scroll, resize, a new
  // layout or a new page of items.
  const schedule = useCallback(() => {
    if (frameRef.current) return;
    frameRef.current = requestAnimationFrame(() => {
      frameRef.current = 0;
      const scroller = scrollRef.current;
      const pool = poolRef.current;
      const current = layoutRef.current;
      if (!scroller || !pool || !current) return;

      const top = scroller.scrollTop;
      const viewport = scroller.clientHeight;
      const startRow = rowAt(current.rowTops, current.rows, top - OVERSCAN);
      const endRow = rowAt(
        current.rowTops,
        current.rows,
        top + viewport + OVERSCAN,
      );
      pool.sync(startRow, endRow, current);

      const scrollable = Math.max(current.totalHeight - viewport, 1);
      scrubberRef.current?.setPosition(
        top / scrollable,
        viewport / Math.max(current.totalHeight, 1),
      );
    });
  }, []);

  useEffect(() => {
    const container = contentRef.current;
    if (!container) return;
    const pool = new TilePool({
      container,
      onSelect: (id, modifiers) => handlers.current.click(id, modifiers),
      onContext: (id, x, y) => {
        // Right-clicking outside the selection selects what was clicked
        // first, so the menu always acts on what is under the pointer.
        if (!selectionRef.current.isSelected(id)) {
          selectionRef.current.click(id, {
            ctrlKey: false,
            metaKey: false,
            shiftKey: false,
          });
        }
        setMenu({ x, y, itemId: id });
      },
      onActivate: (id) => handlers.current.activate(id),
    });
    poolRef.current = pool;
    return () => {
      pool.destroy();
      poolRef.current = null;
    };
  }, []);

  const selectionRef = useRef(selection);
  selectionRef.current = selection;

  useEffect(() => {
    const scroller = scrollRef.current;
    if (!scroller) return;
    const observer = new ResizeObserver(([entry]) => {
      setWidth(entry.contentRect.width);
      schedule();
    });
    observer.observe(scroller);
    return () => observer.disconnect();
  }, [schedule]);

  useEffect(() => {
    poolRef.current?.setItems(items, thumbsDir, spritesDir);
    schedule();
  }, [items, thumbsDir, spritesDir, schedule]);

  useEffect(() => {
    if (contentRef.current && layout) {
      contentRef.current.style.height = `${layout.totalHeight}px`;
    }
    schedule();
  }, [layout, schedule]);

  useEffect(() => {
    poolRef.current?.setSelected(selection.selected);
  }, [selection.selected]);

  useEffect(() => {
    poolRef.current?.setCurrent(selection.current);
  }, [selection.current]);

  useEffect(() => {
    poolRef.current?.retryMissing();
  }, [refreshToken]);

  useEffect(() => {
    return () => {
      // StrictMode double-invokes effects in development: mount, cleanup,
      // mount again, synchronously. That cleanup cancels the real frame
      // `schedule()` already armed during the first mount — if this did not
      // also reset `frameRef`, it would be left holding a dead handle
      // forever, and every later `schedule()` call would see it as truthy
      // and silently never arm another frame. Nothing would ever repaint.
      if (frameRef.current) {
        cancelAnimationFrame(frameRef.current);
        frameRef.current = 0;
      }
    };
  }, []);

  const jump = useCallback((fraction: number) => {
    const scroller = scrollRef.current;
    const current = layoutRef.current;
    if (!scroller || !current) return;
    const scrollable = Math.max(
      current.totalHeight - scroller.clientHeight,
      0,
    );
    scroller.scrollTop = fraction * scrollable;
  }, []);

  return (
    <div className="flex min-h-0 min-w-0 flex-1">
      <div
        ref={scrollRef}
        className="grid-scroll min-w-0 flex-1 overflow-y-auto overflow-x-hidden"
        style={{ padding: PADDING }}
        onScroll={schedule}
        onClick={(event) => {
          if (event.target === event.currentTarget) selection.clear();
        }}
        onContextMenu={(event) => {
          // Only the background — a tile stops the event itself.
          event.preventDefault();
          setMenu({ x: event.clientX, y: event.clientY, itemId: null });
        }}
      >
        <div ref={contentRef} className="relative w-full" />
        {items.length === 0 && (
          <div className="pointer-events-none flex h-full items-center justify-center text-fg-dim">
            {empty ?? "Nothing here yet."}
          </div>
        )}
      </div>

      <PointMenu at={menu} onClose={() => setMenu(null)}>
        {menu && renderMenu(menu)}
      </PointMenu>

      <Scrubber ref={scrubberRef} items={items} layout={layout} onJump={jump} />
    </div>
  );
}
