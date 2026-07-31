import { useCallback, useEffect, useRef, useState } from "react";

import type { GridItem } from "../../lib/types";
import type { LayoutResult } from "./layoutWorker";
import { Scrubber, type ScrubberHandle } from "./Scrubber";
import { TilePool } from "./Tile";
import { rowAt, useJustifiedLayout } from "./useJustifiedLayout";

/** Matches the mockup: 8px around the grid, 4px between tiles. */
const PADDING = 8;
const GAP = 4;
/** Rows are cheap; a screen of overscan either side hides decode latency. */
const OVERSCAN = 600;

interface GridProps {
  items: GridItem[];
  thumbsDir: string;
  spritesDir: string;
  tileHeight: number;
  selectedId: number | null;
  onSelect: (id: number | null) => void;
  /** Bumped while indexing so tiles whose thumbnail did not exist yet retry. */
  refreshToken: number;
}

export function Grid({
  items,
  thumbsDir,
  spritesDir,
  tileHeight,
  selectedId,
  onSelect,
  refreshToken,
}: GridProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const poolRef = useRef<TilePool | null>(null);
  const scrubberRef = useRef<ScrubberHandle>(null);
  const layoutRef = useRef<LayoutResult | null>(null);
  const frameRef = useRef(0);

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
    const pool = new TilePool({ container, onSelect });
    poolRef.current = pool;
    return () => {
      pool.destroy();
      poolRef.current = null;
    };
  }, [onSelect]);

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
    poolRef.current?.setSelected(selectedId);
  }, [selectedId]);

  useEffect(() => {
    poolRef.current?.retryMissing();
  }, [refreshToken]);

  useEffect(() => {
    return () => {
      if (frameRef.current) cancelAnimationFrame(frameRef.current);
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
          if (event.target === event.currentTarget) onSelect(null);
        }}
      >
        <div ref={contentRef} className="relative w-full" />
        {items.length === 0 && (
          <div className="flex h-full items-center justify-center text-fg-dim">
            Nothing here yet.
          </div>
        )}
      </div>
      <Scrubber ref={scrubberRef} items={items} layout={layout} onJump={jump} />
    </div>
  );
}
