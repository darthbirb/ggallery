/** Small labelled pills: folder status, tags, counts. No domain knowledge —
 *  the caller decides what the words mean. */

import type { ReactNode } from "react";

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
      className={`inline-flex max-w-full items-center gap-1 rounded-full border px-2 py-[1px] text-[12px] ${
        muted
          ? "border-line-soft text-fg-dim"
          : "border-line text-fg-mid"
      } ${className ?? ""}`}
      style={colour ? { borderColor: colour, color: colour } : undefined}
    >
      <span className="truncate">{children}</span>
      {onRemove && (
        <button
          type="button"
          aria-label={removeLabel ?? "Remove"}
          onClick={onRemove}
          className="text-fg-dim hover:text-danger"
        >
          ×
        </button>
      )}
    </span>
  );
}
