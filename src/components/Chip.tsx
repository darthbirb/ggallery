/** Small labelled pills: folder status, tags, counts. No domain knowledge —
 *  the caller decides what the words mean.
 *
 *  Sized against decision 25 as M2.8b rewrote it: 26px tall, 12px text, and
 *  an 18px remove target. The remove `×` inherits the chip's own colour and
 *  sits at 60% until hovered, rather than going red — a tag is removed all
 *  the time and it is not a destructive act; the drawing reserves red for
 *  things that reach the trash. */

import { X } from "lucide-react";
import type { ReactNode } from "react";

import { cn } from "../lib/utils";

export function Chip({
  children,
  colour,
  onRemove,
  removeLabel,
  muted,
  className,
}: {
  children: ReactNode;
  /** Status chips carry the user's own colour, which is theirs to pick and
   *  therefore never the accent. */
  colour?: string;
  onRemove?: () => void;
  removeLabel?: string;
  /** Inherited tags read greyed, manual ones solid — DESIGN.md §2. */
  muted?: boolean;
  className?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-[26px] max-w-full items-center gap-1.5 rounded-full border pl-2.5 text-12",
        onRemove ? "pr-1" : "pr-2.5",
        muted
          ? "border-line-soft bg-sunk text-fg-dim"
          : "border-line bg-raised text-fg-mid",
        className,
      )}
      style={colour ? { borderColor: colour, color: colour } : undefined}
    >
      <span className="truncate">{children}</span>
      {onRemove && (
        <button
          type="button"
          aria-label={removeLabel ?? "Remove"}
          onClick={onRemove}
          className="grid size-[18px] shrink-0 place-items-center rounded-full text-current opacity-60 hover:bg-white/12 hover:opacity-100"
        >
          <X className="size-[11px]" />
        </button>
      )}
    </span>
  );
}
