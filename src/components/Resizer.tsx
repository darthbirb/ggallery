/**
 * A vertical drag handle between two panels.
 *
 * Mouse-first (PLAN.md §M2.5, "Settled in phase 1"): the panels are resized by
 * dragging their edge, double-clicking resets to the default, and the same
 * widths are editable in Settings for anyone who would rather type a number.
 * Arrow keys move it too, because a control you can focus should do something.
 */

import { useCallback, useEffect, useRef, useState } from "react";

export interface ResizerProps {
  /** Width the drag started from, in pixels. */
  value: number;
  onChange: (width: number) => void;
  min: number;
  max?: number;
  /** Which side of the handle the panel being sized is on. Dragging right
   *  grows a left-hand panel and shrinks a right-hand one. */
  side: "left" | "right";
  onReset?: () => void;
  label: string;
}

const KEY_STEP = 16;

export function Resizer({
  value,
  onChange,
  min,
  max = 900,
  side,
  onReset,
  label,
}: ResizerProps) {
  const [dragging, setDragging] = useState(false);
  const origin = useRef({ x: 0, width: 0 });

  const clamp = useCallback(
    (width: number) => Math.min(Math.max(width, min), max),
    [min, max],
  );

  useEffect(() => {
    if (!dragging) return;

    const onMove = (event: MouseEvent) => {
      const delta = event.clientX - origin.current.x;
      const width = side === "left" ? origin.current.width + delta : origin.current.width - delta;
      onChange(clamp(width));
    };
    const onUp = () => setDragging(false);

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    // A drag that wanders over the grid must not select tiles or show an
    // I-beam on the way past.
    const previousCursor = document.body.style.cursor;
    document.body.style.cursor = "col-resize";
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.cursor = previousCursor;
    };
  }, [dragging, side, onChange, clamp]);

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation="vertical"
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className={`resizer ${dragging ? "is-dragging" : ""}`}
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        origin.current = { x: event.clientX, width: value };
        setDragging(true);
      }}
      onDoubleClick={() => onReset?.()}
      onKeyDown={(event) => {
        const towards = side === "left" ? 1 : -1;
        if (event.key === "ArrowLeft") {
          event.preventDefault();
          onChange(clamp(value - KEY_STEP * towards));
        } else if (event.key === "ArrowRight") {
          event.preventDefault();
          onChange(clamp(value + KEY_STEP * towards));
        } else if (event.key === "Home" && onReset) {
          event.preventDefault();
          onReset();
        }
      }}
    />
  );
}
