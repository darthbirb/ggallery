/**
 * The item, selection and empty-space right-click menus.
 *
 * "Complete, not a subset" (locked decision 23), audited against the list
 * docs/DESIGN.md §8 says must exist visibly: select all, invert, clear;
 * favourite; delete; reveal in Explorer; open with; copy file; copy path;
 * fold the navigation panel. Blur and negate-a-query-term belong to M9 and
 * M3 — the features themselves do not exist yet.
 *
 * One item and forty behave the same way: a right-click on something already
 * selected acts on the whole selection, a right-click on something else
 * selects it first. The menu below is written once for both, and the caller
 * hands it the ids it should act on.
 */

import { MenuItem, MenuLabel, MenuSeparator } from "../../components/Menu";
import type { GridItem } from "../../lib/types";
import { useDialogs } from "./Dialogs";
import { useOperations } from "./operations";

export interface ItemMenuProps {
  /** Everything this menu acts on — one item, or the whole selection. */
  itemIds: number[];
  /** The item actually clicked, when there is exactly one — the operations
   *  that only make sense singly (reveal, open with, set as cover). */
  item: GridItem | null;
  /** The folder currently in view, when the grid is showing one. Only then
   *  can "set as this folder's cover" mean anything. */
  folder: { id: number; title: string } | null;
  /** Show this item in the pane. */
  onPreview: (itemId: number) => void;
}

export function ItemMenu({ itemIds, item, folder, onPreview }: ItemMenuProps) {
  const ops = useOperations();
  const dialogs = useDialogs();
  const many = itemIds.length > 1;
  const single = !many && item ? item : null;
  // A mixed selection reads as "not favourited yet", so one click makes it
  // uniform rather than toggling half of it back.
  const makeFavorite = !(single?.favorite ?? false);

  return (
    <>
      <MenuLabel>{many ? `${itemIds.length} items` : (item?.name ?? "Item")}</MenuLabel>

      {single && (
        <MenuItem onSelect={() => onPreview(single.id)} shortcut="Enter">
          Show in the pane
        </MenuItem>
      )}

      <MenuItem
        onSelect={() => ops.setFavorite(itemIds, makeFavorite)}
        shortcut="F"
      >
        {makeFavorite ? "Favourite" : "Remove favourite"}
      </MenuItem>
      <MenuItem onSelect={() => dialogs.tagItems(itemIds)}>Add tag…</MenuItem>

      <MenuSeparator />

      <MenuItem onSelect={() => dialogs.moveItems(itemIds)}>Move to…</MenuItem>

      {single && folder && (
        <MenuItem onSelect={() => ops.setFolderCover(folder.id, single.id)}>
          Set as {folder.title}&rsquo;s cover
        </MenuItem>
      )}

      <MenuSeparator />

      {single && (
        <>
          <MenuItem onSelect={() => ops.revealItem(single.id)}>
            Reveal in Explorer
          </MenuItem>
          <MenuItem onSelect={() => ops.openItem(single.id)}>
            Open with the default app
          </MenuItem>
          <MenuItem onSelect={() => ops.copyItemFile(single.id)} shortcut="Ctrl+C">
            Copy file
          </MenuItem>
          <MenuItem onSelect={() => ops.copyItemPath(single.id)}>Copy path</MenuItem>
          <MenuSeparator />
        </>
      )}

      <MenuItem danger onSelect={() => dialogs.deleteItems(itemIds)} shortcut="Del">
        Delete…
      </MenuItem>
    </>
  );
}

export interface EmptyMenuProps {
  /** The folder in view, so "new folder" lands somewhere predictable. */
  folder: { id: number; title: string } | null;
  hasItems: boolean;
  hasSelection: boolean;
  onSelectAll: () => void;
  onInvert: () => void;
  onClear: () => void;
  onNewFolder: () => void;
  bandExpanded: boolean;
  onToggleBand: () => void;
  paneOpen: boolean;
  onTogglePane: () => void;
}

/** Right-click on the grid's background. */
export function EmptyMenu({
  folder,
  hasItems,
  hasSelection,
  onSelectAll,
  onInvert,
  onClear,
  onNewFolder,
  bandExpanded,
  onToggleBand,
  paneOpen,
  onTogglePane,
}: EmptyMenuProps) {
  return (
    <>
      <MenuItem onSelect={onSelectAll} disabled={!hasItems} shortcut="Ctrl+A">
        Select all
      </MenuItem>
      <MenuItem onSelect={onInvert} disabled={!hasItems}>
        Invert selection
      </MenuItem>
      <MenuItem onSelect={onClear} disabled={!hasSelection} shortcut="Esc">
        Clear selection
      </MenuItem>

      <MenuSeparator />

      <MenuItem onSelect={onNewFolder}>
        New folder in {folder ? folder.title : "the top level"}…
      </MenuItem>

      <MenuSeparator />

      <MenuItem onSelect={onToggleBand}>
        {bandExpanded ? "Collapse folder details" : "Expand folder details"}
      </MenuItem>
      <MenuItem onSelect={onTogglePane}>
        {paneOpen ? "Close the pane" : "Open the pane"}
      </MenuItem>
    </>
  );
}
