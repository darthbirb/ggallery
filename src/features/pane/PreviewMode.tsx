/**
 * Preview mode — **N panes, not one**.
 *
 * This is the app's only comparison surface, and it is the same mechanism at
 * every size: one pane is the viewer, two with synced pan and zoom are
 * compression review (M6) and duplicate comparison (M7), and up to twelve are
 * multi-view (M10). So it takes a list of slots and lays them out, rather than
 * rendering "the selected item" and being generalised later — which is how
 * you end up with a single-pane viewer plus three bespoke comparison screens,
 * exactly what the design collapsed.
 *
 * What M2.5a builds: the slot list, the adaptive layout, per-slot item
 * rendering, navigation through the current filter, the filmstrip, and the
 * collapsible details. What later milestones add: more than one slot, synced
 * pan/zoom and a shared timeline across slots, and audio soloing.
 */

import { useEffect, useState } from "react";

import { IconButton } from "../../components/Button";
import { Tooltip } from "../../components/Tooltip";
import * as ipc from "../../lib/ipc";
import type { GridItem, ItemDetail } from "../../lib/types";
import { Details } from "./Details";
import { ItemView } from "./ItemView";

/** One pane's worth of preview. `itemId` null renders the empty state. */
export interface PreviewSlot {
  key: string;
  itemId: number | null;
}

/**
 * The layout docs/DESIGN.md §2 specifies: 2 side by side, 3–4 as 2×2, 5–6 as
 * 3×2, 7–9 as 3×3, 10–12 as 4×3. Pure, so it is testable on its own and
 * cannot drift when M10 starts asking it for twelve.
 */
export function paneGrid(count: number): { columns: number; rows: number } {
  if (count <= 1) return { columns: 1, rows: 1 };
  if (count === 2) return { columns: 2, rows: 1 };
  if (count <= 4) return { columns: 2, rows: 2 };
  if (count <= 6) return { columns: 3, rows: 2 };
  if (count <= 9) return { columns: 3, rows: 3 };
  return { columns: 4, rows: 3 };
}

export interface PreviewModeProps {
  slots: PreviewSlot[];
  /** The current filter, in the grid's order — what the chevrons and the
   *  filmstrip move through. */
  items: GridItem[];
  thumbsDir: string;
  /** Move the current item by ±1 through `items`. */
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
  detailsExpanded: boolean;
  onDetailsExpandedChange: (expanded: boolean) => void;
  /** Bumped when something that could change an item lands. */
  refreshToken: number;
}

export function PreviewMode({
  slots,
  items,
  thumbsDir,
  onStep,
  onPick,
  detailsExpanded,
  onDetailsExpandedChange,
  refreshToken,
}: PreviewModeProps) {
  const { columns, rows } = paneGrid(slots.length);
  // Details describe one item; with several panes open they belong to the
  // first, which is the one M6 and M7 treat as the subject.
  const leadId = slots[0]?.itemId ?? null;
  const lead = useItemDetail(leadId, refreshToken);

  if (slots.length === 0 || slots.every((slot) => slot.itemId === null)) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-1 px-6 text-center text-fg-dim">
        <span className="text-[20px]">◻</span>
        <p className="max-w-[30ch] text-[12px]">
          Nothing selected. Click something in the grid and it appears here.
        </p>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div
        className="grid min-h-0 flex-1 gap-1 p-1"
        style={{
          gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))`,
          gridTemplateRows: `repeat(${rows}, minmax(0, 1fr))`,
        }}
      >
        {slots.map((slot) => (
          <Slot key={slot.key} itemId={slot.itemId} refreshToken={refreshToken} />
        ))}
      </div>

      <Filmstrip
        items={items}
        thumbsDir={thumbsDir}
        currentId={leadId}
        onStep={onStep}
        onPick={onPick}
      />

      {lead && (
        <Details
          item={lead}
          expanded={detailsExpanded}
          onExpandedChange={onDetailsExpandedChange}
          refreshToken={refreshToken}
        />
      )}
    </div>
  );
}

function Slot({ itemId, refreshToken }: { itemId: number | null; refreshToken: number }) {
  const detail = useItemDetail(itemId, refreshToken);
  if (!detail) {
    return <div className="min-h-0 rounded-[4px] bg-ground" />;
  }
  return (
    <div className="min-h-0 overflow-hidden rounded-[4px] bg-ground">
      <ItemView item={detail} />
    </div>
  );
}

function useItemDetail(itemId: number | null, refreshToken: number): ItemDetail | null {
  const [detail, setDetail] = useState<ItemDetail | null>(null);

  useEffect(() => {
    if (itemId === null) {
      setDetail(null);
      return;
    }
    let cancelled = false;
    void ipc
      .getItem(itemId)
      .then((next) => !cancelled && setDetail(next))
      .catch(() => !cancelled && setDetail(null));
    return () => {
      cancelled = true;
    };
  }, [itemId, refreshToken]);

  return detail;
}

/** Position in the current filter, and a way to jump. */
function Filmstrip({
  items,
  thumbsDir,
  currentId,
  onStep,
  onPick,
}: {
  items: GridItem[];
  thumbsDir: string;
  currentId: number | null;
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
}) {
  const at = currentId === null ? -1 : items.findIndex((item) => item.id === currentId);

  return (
    <div className="flex shrink-0 items-center gap-1 border-t border-line-soft px-1 py-1">
      <Tooltip label="Previous" side="top">
        <IconButton aria-label="Previous" disabled={at <= 0} onClick={() => onStep(-1)}>
          ‹
        </IconButton>
      </Tooltip>

      <div className="flex min-w-0 flex-1 gap-1 overflow-x-auto">
        {items.map((item) => (
          <button
            key={item.id}
            type="button"
            aria-label={item.name}
            aria-current={item.id === currentId}
            onClick={() => onPick(item.id)}
            ref={(node) => {
              if (item.id === currentId) {
                node?.scrollIntoView({ block: "nearest", inline: "center" });
              }
            }}
            className={`h-9 w-12 shrink-0 overflow-hidden rounded-[3px] border ${
              item.id === currentId ? "border-accent" : "border-transparent opacity-60"
            }`}
          >
            <img
              src={ipc.assetUrl(thumbsDir, item.thumb)}
              alt=""
              draggable={false}
              className="h-full w-full object-cover"
            />
          </button>
        ))}
      </div>

      <span className="shrink-0 px-1 font-mono text-[11px] tabular-nums text-fg-dim">
        {at >= 0 ? `${at + 1}/${items.length}` : `–/${items.length}`}
      </span>

      <Tooltip label="Next" side="top">
        <IconButton
          aria-label="Next"
          disabled={at === -1 || at >= items.length - 1}
          onClick={() => onStep(1)}
        >
          ›
        </IconButton>
      </Tooltip>
    </div>
  );
}
