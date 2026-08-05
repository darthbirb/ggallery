/**
 * The pane's Grid mode — a second grid, scoped anywhere in the library, with
 * its own tile size. DESIGN.md §2 *Grid mode*.
 *
 * One of the app's three drop targets (DESIGN.md §*Drops*): drag items onto
 * it and they move into whatever folder it is currently showing. There is
 * no navigation control specified beyond "scoped anywhere" — this reuses the
 * same flat title/path picker `Dialogs.tsx` already built for "Move to…",
 * since that is the one existing "choose a folder from the whole library"
 * surface rather than a second one invented for this.
 *
 * Subfolders never appear here, the same rule the main grid follows — this
 * is media, not structure.
 */

import { FolderOpen, Square } from "lucide-react";
import { useEffect, useState } from "react";

import { Breadcrumb } from "../../components/Breadcrumb";
import { MenuItem, MenuLabel } from "../../components/Menu";
import { Slider } from "../../components/ui/slider";
import { ancestorTitles } from "../../lib/folders";
import * as ipc from "../../lib/ipc";
import type { FolderNode, GridItem } from "../../lib/types";
import { dropIsValid, resolveDrop, useDnd } from "../../state/dnd";
import { useSelection } from "../../state/selection";
import { TILE_SIZES, type PaneMode } from "../../state/ui";
import { Grid, type GridMenuTarget } from "../grid/Grid";
import { FolderPickerDialog } from "../menus/Dialogs";
import { ItemMenu } from "../menus/ItemMenu";
import { useOperations } from "../menus/operations";
import { PaneFrame } from "./PaneFrame";

export interface GridModeProps {
  mode: PaneMode;
  onModeChange: (mode: PaneMode) => void;
  onClose: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;
  folders: FolderNode[];
  refreshToken: number;
  thumbsDir: string;
  spritesDir: string;
  onPreview: (itemId: number) => void;
}

export function GridMode({
  mode,
  onModeChange,
  onClose,
  maximised,
  onMaximisedChange,
  folders,
  refreshToken,
  thumbsDir,
  spritesDir,
  onPreview,
}: GridModeProps) {
  const ops = useOperations();
  const { dragging } = useDnd();
  const [folderId, setFolderId] = useState<number | null>(null);
  const [tileHeight, setTileHeight] = useState(TILE_SIZES[1]);
  const [items, setItems] = useState<GridItem[]>([]);
  const [picking, setPicking] = useState(false);
  const selection = useSelection(items);

  const folder = folderId !== null ? (folders.find((node) => node.id === folderId) ?? null) : null;
  // A folder this view was showing that got deleted, or moved out from under
  // it, falls back to Everything rather than going on showing stale rows.
  useEffect(() => {
    if (folderId !== null && !folder) setFolderId(null);
  }, [folderId, folder]);

  useEffect(() => {
    let cancelled = false;
    ipc
      .listItems(folderId, false, true)
      .then((rows) => {
        if (!cancelled) setItems(rows);
      })
      .catch(() => {
        if (!cancelled) setItems([]);
      });
    return () => {
      cancelled = true;
    };
  }, [folderId, refreshToken]);

  // Everything has no real destination to file into — the drop is only
  // meaningful once this view is actually showing a folder.
  const accepting = dragging !== null && folder !== null && dropIsValid(dragging, folder.id);

  return (
    <PaneFrame
      mode={mode}
      onModeChange={onModeChange}
      maximised={maximised}
      onMaximisedChange={onMaximisedChange}
      onClose={onClose}
      header={
        <button
          type="button"
          onClick={() => setPicking(true)}
          className="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-[4px] px-1.5 text-left hover:bg-hover"
        >
          <FolderOpen className="size-4 shrink-0 text-fg-dim" />
          {folder ? (
            <Breadcrumb titles={ancestorTitles(folders, folder.id)} />
          ) : (
            <span className="truncate text-fg-mid">Everything</span>
          )}
        </button>
      }
    >
      <div className="flex min-h-0 flex-1 flex-col">
        <div
          onDragOver={(event) => {
            if (accepting) event.preventDefault();
          }}
          onDrop={(event) => {
            event.preventDefault();
            if (dragging && accepting && folder) resolveDrop(dragging, folder, ops);
          }}
          className={
            accepting
              ? "flex min-h-0 flex-1 outline outline-2 -outline-offset-2 outline-accent"
              : "flex min-h-0 flex-1"
          }
        >
          <Grid
            items={items}
            thumbsDir={thumbsDir}
            spritesDir={spritesDir}
            tileHeight={tileHeight}
            selection={selection}
            refreshToken={refreshToken}
            onActivate={onPreview}
            empty={folder ? `Nothing in ${folder.title}.` : "Nothing here yet."}
            renderMenu={(target: GridMenuTarget) =>
              target.itemId === null ? (
                <>
                  <MenuLabel>{items.length === 0 ? "Nothing here" : `${items.length} items`}</MenuLabel>
                  <MenuItem onSelect={selection.selectAll} disabled={items.length === 0} shortcut="Ctrl+A">
                    Select all
                  </MenuItem>
                  <MenuItem onSelect={selection.invert} disabled={items.length === 0}>
                    Invert selection
                  </MenuItem>
                  <MenuItem onSelect={selection.clear} disabled={selection.count === 0} shortcut="Esc">
                    Clear selection
                  </MenuItem>
                </>
              ) : (
                <ItemMenu
                  itemIds={
                    selection.count > 1 && selection.isSelected(target.itemId)
                      ? [...selection.selected]
                      : [target.itemId]
                  }
                  item={items.find((item) => item.id === target.itemId) ?? null}
                  folder={folder}
                  onPreview={onPreview}
                />
              )
            }
          />
        </div>

        <div className="flex h-9 shrink-0 items-center gap-2 border-t border-line px-2">
          <span className="text-fg-dim">Tile size</span>
          <Slider
            aria-label="Tile size"
            className="w-24"
            min={0}
            max={TILE_SIZES.length - 1}
            value={[Math.max(TILE_SIZES.indexOf(tileHeight), 0)]}
            onValueChange={([index]) => setTileHeight(TILE_SIZES[index])}
          />
          <Square aria-hidden fill="currentColor" className="size-4 shrink-0 text-fg-dim" />
          <span className="ml-auto truncate font-mono tabular-nums text-fg-dim">
            {items.length} item{items.length === 1 ? "" : "s"}
          </span>
        </div>
      </div>

      {picking && (
        <FolderPickerDialog
          title="Show in Grid mode…"
          folders={folders}
          allowTopLevel
          topLabel="Everything"
          onClose={() => setPicking(false)}
          onPick={(dest) => setFolderId(dest?.id ?? null)}
        />
      )}
    </PaneFrame>
  );
}
