/**
 * A drag handle between two panels.
 *
 * Mouse-first (PLAN.md §M2.5, "Settled in phase 1"): panels are resized by
 * dragging their edge and double-clicking resets to the default. Arrow keys
 * move it too, because a control you can focus should do something.
 *
 * Works on either axis. `side` names where the panel being sized sits
 * relative to the handle, which is all the caller has to think about:
 * dragging right grows a `left` panel and shrinks a `right` one, dragging
 * down grows a `top` panel and shrinks a `bottom` one.
 */

import { useCallback, useEffect, useRef, useState } from "react";

export interface ResizerProps {
  /** Size the drag starts from, in pixels — width on the x axis, height on
   *  the y axis. */
  value: number;
  onChange: (size: number) => void;
  min: number;
  max?: number;
  /** Which side of the handle the panel being sized is on. */
  side: "left" | "right" | "top" | "bottom";
  onReset?: () => void;
  label: string;
  /** Fires as a drag starts and ends. A panel whose size is fed through a CSS
   *  transition (folding, maximising) must drop that transition for the
   *  duration of a live drag — otherwise every mousemove queues another
   *  eased animation and the edge trails behind the cursor instead of
   *  tracking it, which is the whole difference between this handle feeling
   *  laggy and the filmstrip's (untransitioned) one feeling instant. */
  onDraggingChange?: (dragging: boolean) => void;
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
  onDraggingChange,
}: ResizerProps) {
  const [dragging, setDragging] = useState(false);
  const origin = useRef({ at: 0, size: 0 });

  const vertical = side === "left" || side === "right";
  /** +1 when dragging away from the origin grows the panel. */
  const towards = side === "left" || side === "top" ? 1 : -1;

  const clamp = useCallback(
    (size: number) => Math.min(Math.max(size, min), max),
    [min, max],
  );

  useEffect(() => {
    if (!dragging) return;

    const onMove = (event: MouseEvent) => {
      const at = vertical ? event.clientX : event.clientY;
      const delta = (at - origin.current.at) * towards;
      onChange(clamp(origin.current.size + delta));
    };
    const onUp = () => {
      setDragging(false);
      onDraggingChange?.(false);
    };

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    // A drag that wanders over the grid must not select tiles or show an
    // I-beam on the way past.
    const previousCursor = document.body.style.cursor;
    document.body.style.cursor = vertical ? "col-resize" : "row-resize";
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.cursor = previousCursor;
    };
  }, [dragging, vertical, towards, onChange, clamp]);

  return (
    <div
      role="separator"
      aria-label={label}
      aria-orientation={vertical ? "vertical" : "horizontal"}
      aria-valuenow={Math.round(value)}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      className={`resizer ${vertical ? "is-x" : "is-y"} ${dragging ? "is-dragging" : ""}`}
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        origin.current = {
          at: vertical ? event.clientX : event.clientY,
          size: value,
        };
        setDragging(true);
        onDraggingChange?.(true);
      }}
      onDoubleClick={() => onReset?.()}
      onKeyDown={(event) => {
        const less = vertical ? "ArrowLeft" : "ArrowUp";
        const more = vertical ? "ArrowRight" : "ArrowDown";
        if (event.key === less) {
          event.preventDefault();
          onChange(clamp(value - KEY_STEP * towards));
        } else if (event.key === more) {
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
