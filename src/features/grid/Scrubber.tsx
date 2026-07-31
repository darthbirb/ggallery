import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
  useState,
} from "react";

import { monthLabel } from "../../lib/format";
import type { GridItem } from "../../lib/types";
import type { LayoutResult } from "./layoutWorker";

export interface ScrubberHandle {
  /** Called from the grid's rAF loop — never through React state. */
  setPosition(fraction: number, viewportFraction: number): void;
}

interface ScrubberProps {
  items: GridItem[];
  layout: LayoutResult | null;
  onJump: (fraction: number) => void;
}

interface Tick {
  fraction: number;
  label: string;
  isYear: boolean;
}

/** Minimum pixels between two labels before one is dropped. */
const MIN_TICK_GAP = 16;

export const Scrubber = forwardRef<ScrubberHandle, ScrubberProps>(
  function Scrubber({ items, layout, onJump }, ref) {
    const stripRef = useRef<HTMLDivElement>(null);
    const thumbRef = useRef<HTMLDivElement>(null);
    const [height, setHeight] = useState(0);

    // Drag coalescing: M0 found 58% of scrubber-drag frames over 32ms, several
    // past 100ms, while the same relayout path driven by the size slider
    // stayed clean — the cost was the per-jump repaint, not the layout. So a
    // drag records where it wants to be and jumps once per animation frame.
    const pending = useRef<number | null>(null);
    const frame = useRef(0);
    const dragging = useRef(false);

    useImperativeHandle(
      ref,
      () => ({
        setPosition(fraction, viewportFraction) {
          const thumb = thumbRef.current;
          if (!thumb) return;
          const size = Math.max(viewportFraction * 100, 3);
          thumb.style.height = `${size}%`;
          thumb.style.top = `${Math.min(Math.max(fraction, 0), 1) * (100 - size)}%`;
        },
      }),
      [],
    );

    useEffect(() => {
      const strip = stripRef.current;
      if (!strip) return;
      const observer = new ResizeObserver(([entry]) => {
        setHeight(entry.contentRect.height);
      });
      observer.observe(strip);
      return () => observer.disconnect();
    }, []);

    const ticks = useMemo(
      () => buildTicks(items, layout, height),
      [items, layout, height],
    );

    useEffect(() => {
      return () => {
        if (frame.current) cancelAnimationFrame(frame.current);
      };
    }, []);

    const queueJump = (event: React.PointerEvent<HTMLDivElement>) => {
      const strip = stripRef.current;
      if (!strip) return;
      const rect = strip.getBoundingClientRect();
      const fraction = (event.clientY - rect.top) / Math.max(rect.height, 1);
      pending.current = Math.min(Math.max(fraction, 0), 1);
      if (frame.current) return;
      frame.current = requestAnimationFrame(() => {
        frame.current = 0;
        if (pending.current === null) return;
        onJump(pending.current);
        pending.current = null;
      });
    };

    return (
      <div
        ref={stripRef}
        className="relative w-[26px] shrink-0 select-none border-l border-line-soft bg-panel font-mono text-[9px] text-fg-dim"
        onPointerDown={(event) => {
          dragging.current = true;
          event.currentTarget.setPointerCapture(event.pointerId);
          queueJump(event);
        }}
        onPointerMove={(event) => {
          if (dragging.current) queueJump(event);
        }}
        onPointerUp={(event) => {
          dragging.current = false;
          event.currentTarget.releasePointerCapture(event.pointerId);
        }}
      >
        <div
          ref={thumbRef}
          className="pointer-events-none absolute inset-x-[2px] top-0 h-[3%] rounded-[3px] bg-accent opacity-25"
        />
        {ticks.map((tick) => (
          <div
            key={`${tick.label}-${tick.fraction}`}
            className={`pointer-events-none absolute w-full text-center ${
              tick.isYear ? "text-fg-mid" : "opacity-50"
            }`}
            style={{ top: `${tick.fraction * 100}%` }}
          >
            {tick.label}
          </div>
        ))}
      </div>
    );
  },
);

/**
 * One tick per month boundary, positioned where that month actually starts in
 * the laid-out grid rather than by item index, then thinned until the labels
 * stop colliding.
 */
function buildTicks(
  items: GridItem[],
  layout: LayoutResult | null,
  height: number,
): Tick[] {
  if (!layout || layout.rows === 0 || items.length === 0 || height <= 0) {
    return [];
  }

  const candidates: Tick[] = [];
  let lastKey = "";
  let lastYear = Number.NaN;

  for (let index = 0; index < items.length; index += 1) {
    const date = new Date(items[index].at * 1000);
    const key = `${date.getFullYear()}-${date.getMonth()}`;
    if (key === lastKey) continue;
    lastKey = key;

    const isYear = date.getFullYear() !== lastYear;
    lastYear = date.getFullYear();
    const row = rowForItem(layout, index);
    candidates.push({
      fraction: layout.rowTops[row] / Math.max(layout.totalHeight, 1),
      label: isYear ? String(date.getFullYear()) : monthLabel(date.getMonth()),
      isYear,
    });
  }

  const kept: Tick[] = [];
  let lastPixel = -Infinity;
  for (const tick of candidates) {
    const pixel = tick.fraction * height;
    if (pixel - lastPixel < MIN_TICK_GAP) continue;
    kept.push(tick);
    lastPixel = pixel;
  }
  return kept;
}

/** Which row an item index landed in. */
function rowForItem(layout: LayoutResult, index: number): number {
  let low = 0;
  let high = layout.rows - 1;
  let found = 0;
  while (low <= high) {
    const mid = (low + high) >> 1;
    if (layout.rowStart[mid] <= index) {
      found = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return found;
}
