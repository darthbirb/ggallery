/** Small labelled pills: folder status, tags, counts. No domain knowledge —
 *  the caller decides what the words mean.
 *
 *  Sized against decision 25: 28px tall with a real 20px remove target rather
 *  than a 3px cross, and 13px text. */

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
        "inline-flex h-7 max-w-full items-center gap-1 rounded-full border pl-2.5 text-[13px]",
        onRemove ? "pr-1" : "pr-2.5",
        muted
          ? "border-line-soft bg-ground text-fg-dim"
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
          className="grid size-5 shrink-0 place-items-center rounded-full text-fg-dim hover:bg-danger/20 hover:text-danger"
        >
          <X className="size-3.5" />
        </button>
      )}
    </span>
  );
}
