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
 * Vertically it is: the pane header (which this mode fills with the item's
 * name), the details body when open, the media, and the filmstrip pinned to
 * the bottom edge. What later milestones add: more than one slot, synced
 * pan/zoom and a shared timeline across slots, and audio soloing.
 */

import { ChevronLeft, ChevronRight, ImageOff } from "lucide-react";
import { useEffect, useState } from "react";

import { Resizer } from "../../components/Resizer";
import { Tooltip } from "../../components/Tooltip";
import { IconButton } from "../../components/ui/button";
import * as ipc from "../../lib/ipc";
import type { GridItem, ItemDetail } from "../../lib/types";
import { cn } from "../../lib/utils";
import { FILMSTRIP_MAX, FILMSTRIP_MIN } from "../../state/ui";
import { DetailsBody, DetailsHeader } from "./Details";
import { ItemView } from "./ItemView";
import { PaneFrame } from "./PaneFrame";

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
  filmstripHeight: number;
  onFilmstripHeightChange: (height: number) => void;
  onResetFilmstripHeight: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;
  onClose: () => void;
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
  filmstripHeight,
  onFilmstripHeightChange,
  onResetFilmstripHeight,
  maximised,
  onMaximisedChange,
  onClose,
  refreshToken,
}: PreviewModeProps) {
  const { columns, rows } = paneGrid(slots.length);
  // Details describe one item; with several panes open they belong to the
  // first, which is the one M6 and M7 treat as the subject.
  const leadId = slots[0]?.itemId ?? null;
  const lead = useItemDetail(leadId, refreshToken);
  const empty = slots.length === 0 || slots.every((slot) => slot.itemId === null);

  return (
    <PaneFrame
      header={
        lead && !empty ? (
          <DetailsHeader
            item={lead}
            expanded={detailsExpanded}
            onExpandedChange={onDetailsExpandedChange}
          />
        ) : undefined
      }
      maximised={maximised}
      onMaximisedChange={onMaximisedChange}
      onClose={onClose}
    >
      {empty ? (
        <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-2 px-6 text-center text-fg-dim">
          <ImageOff className="size-7" />
          <p>Nothing selected.</p>
        </div>
      ) : (
        <>
          {/* Opened from the header, and growing **downwards** from it — the
              media gives way, and the filmstrip never moves. */}
          {lead && detailsExpanded && (
            <DetailsBody item={lead} refreshToken={refreshToken} />
          )}

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

          <Resizer
            label="Filmstrip height"
            side="bottom"
            value={filmstripHeight}
            min={FILMSTRIP_MIN}
            max={FILMSTRIP_MAX}
            onChange={onFilmstripHeightChange}
            onReset={onResetFilmstripHeight}
          />

          <Filmstrip
            items={items}
            thumbsDir={thumbsDir}
            currentId={leadId}
            height={filmstripHeight}
            onStep={onStep}
            onPick={onPick}
          />
        </>
      )}
    </PaneFrame>
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

/** Room for an overlaid chevron at each end, so a thumbnail scrolling past
 *  is never hidden underneath one at rest. */
const STRIP_INSET = 44;

/**
 * Position in the current filter, and a way to jump.
 *
 * The scroll container **is** the strip: full width, flush to the pane's
 * bottom edge, so its scrollbar runs the whole way across at the very bottom
 * rather than floating in a padded box. That means the chevrons cannot be
 * siblings competing for the same row — they are overlaid at either end over
 * a fade to the panel colour, with matching padding inside the scroller so
 * nothing ever sits under them at rest.
 *
 * **No position counter.** The strip already shows where you are; `6 / 15` is
 * a number nobody acts on. Only the chevrons remain.
 */
function Filmstrip({
  items,
  thumbsDir,
  currentId,
  height,
  onStep,
  onPick,
}: {
  items: GridItem[];
  thumbsDir: string;
  currentId: number | null;
  height: number;
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
}) {
  const at = currentId === null ? -1 : items.findIndex((item) => item.id === currentId);

  return (
    <div className="relative shrink-0 bg-panel" style={{ height }}>
      <div
        // `pb-3` clears the scrollbar's own 16px channel, so the thumbnails'
        // bottom edge does not sit hard against it — M2.5a.1 made that
        // scrollbar full-width and flush to the bottom, which put the two
        // directly against each other.
        className="flex h-full items-stretch gap-1 overflow-x-auto pb-3 pt-1.5"
        style={{ paddingLeft: STRIP_INSET, paddingRight: STRIP_INSET }}
      >
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
            className={cn(
              "aspect-[7/5] h-full shrink-0 overflow-hidden rounded-[4px] border-2",
              // One state, the accent — the same statement selection makes in
              // the grid (decision 26).
              item.id === currentId
                ? "border-accent"
                : "border-transparent opacity-55 hover:opacity-100",
            )}
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

      {/* The scrollbar owns the bottom of the strip, so the overlays stop
          short of it — `bottom-4` is its 16px channel. */}
      <div className="pointer-events-none absolute bottom-4 left-0 top-0 flex items-center bg-gradient-to-r from-panel via-panel to-transparent pl-1.5 pr-4">
        <Tooltip label="Previous" side="top">
          <IconButton
            aria-label="Previous"
            className="pointer-events-auto"
            disabled={at <= 0}
            onClick={() => onStep(-1)}
          >
            <ChevronLeft />
          </IconButton>
        </Tooltip>
      </div>

      <div className="pointer-events-none absolute bottom-4 right-0 top-0 flex items-center gap-1.5 bg-gradient-to-l from-panel via-panel to-transparent pl-4 pr-1.5">
        <Tooltip label="Next" side="top">
          <IconButton
            aria-label="Next"
            className="pointer-events-auto"
            disabled={at === -1 || at >= items.length - 1}
            onClick={() => onStep(1)}
          >
            <ChevronRight />
          </IconButton>
        </Tooltip>
      </div>
    </div>
  );
}
