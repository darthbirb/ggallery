/**
 * The timeline scrubber — a thin strip down the right edge of the grid, and
 * the grid's only scroll affordance.
 *
 * **No year or month column, and no date at all.** A permanent list of
 * labels is visual noise for something you look at for one second at a time,
 * and it forced the strip wide enough to read text down. M2.5a.1 tried a
 * single date that followed the thumb while dragging instead; M2.5a.2 drops
 * that too, after two passes trying to make it readable next to a thumb held
 * for under a second landed on the same conclusion both times — the position
 * *is* the information (SPEC.md §2).
 *
 * The strip is part of the grid's own width — `SCRUBBER_WIDTH` is exported so
 * the bar beneath the grid can inset by it rather than running underneath.
 */

import { forwardRef, useEffect, useImperativeHandle, useRef } from "react";

/** The same 16px a scrollbar gets — the scrubber is the grid's scroll
 *  affordance, so it should read as one. Exported so the selection bar below
 *  the grid reserves the same column instead of running under it. */
export const SCRUBBER_WIDTH = 16;

export interface ScrubberHandle {
  /** Called from the grid's rAF loop — never through React state. */
  setPosition(fraction: number, viewportFraction: number): void;
}

interface ScrubberProps {
  onJump: (fraction: number) => void;
}

export const Scrubber = forwardRef<ScrubberHandle, ScrubberProps>(
  function Scrubber({ onJump }, ref) {
    const stripRef = useRef<HTMLDivElement>(null);
    const thumbRef = useRef<HTMLDivElement>(null);

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
          const size = Math.max(viewportFraction * 100, 4);
          thumb.style.height = `${size}%`;
          thumb.style.top = `${Math.min(Math.max(fraction, 0), 1) * (100 - size)}%`;
        },
      }),
      [],
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
      const raw = (event.clientY - rect.top) / Math.max(rect.height, 1);
      const fraction = Math.min(Math.max(raw, 0), 1);
      pending.current = fraction;
      if (frame.current) return;
      frame.current = requestAnimationFrame(() => {
        frame.current = 0;
        const at = pending.current;
        if (at === null) return;
        pending.current = null;
        onJump(at);
      });
    };

    const endDrag = (event: React.PointerEvent<HTMLDivElement>) => {
      dragging.current = false;
      event.currentTarget.releasePointerCapture(event.pointerId);
    };

    return (
      <div
        ref={stripRef}
        aria-label="Timeline"
        style={{ width: SCRUBBER_WIDTH }}
        className="scrubber relative shrink-0 select-none"
        onPointerDown={(event) => {
          dragging.current = true;
          event.currentTarget.setPointerCapture(event.pointerId);
          queueJump(event);
        }}
        onPointerMove={(event) => {
          if (dragging.current) queueJump(event);
        }}
        onPointerUp={endDrag}
        onPointerCancel={endDrag}
      >
        <div ref={thumbRef} className="scrubber-thumb top-0 h-[4%]" />
      </div>
    );
  },
);
