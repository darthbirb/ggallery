/**
 * The folder right-click menu — complete, not a subset (locked decision 23).
 *
 * Used by every surface that can show a folder: the navigation tree, the
 * folder band, and (in M2.5b) folder tiles. One definition, so a capability
 * cannot exist in one place and be missing in another — the failure decision
 * 22 was written about.
 */

import { MenuItem, MenuLabel, MenuSeparator, MenuSub } from "../../components/Menu";
import type { ArchetypeInfo, FolderNode, FolderStatusDef } from "../../lib/types";
import { useDialogs } from "./Dialogs";
import { useOperations } from "./operations";

export interface FolderMenuProps {
  folder: FolderNode;
  statuses: FolderStatusDef[];
  archetypes: ArchetypeInfo[];
  /** Show this folder in the grid. */
  onOpen: (folder: FolderNode) => void;
  /** Open the folder band expanded, which is where fields, tags and notes are
   *  edited — the menu points at it rather than duplicating it. */
  onEditDetails: (folder: FolderNode) => void;
}

export function FolderMenu({
  folder,
  statuses,
  archetypes,
  onOpen,
  onEditDetails,
}: FolderMenuProps) {
  const ops = useOperations();
  const dialogs = useDialogs();

  return (
    <>
      <MenuItem onSelect={() => onOpen(folder)}>Open</MenuItem>
      <MenuItem onSelect={() => onEditDetails(folder)}>
        Fields, tags and notes…
      </MenuItem>

      <MenuSeparator />

      <MenuItem onSelect={() => dialogs.newFolder(folder)}>New folder inside…</MenuItem>
      <MenuItem onSelect={() => dialogs.renameFolder(folder)}>Rename…</MenuItem>
      <MenuItem onSelect={() => dialogs.moveFolder(folder)}>Move to…</MenuItem>

      <MenuSeparator />

      <MenuSub label="Status" disabled={statuses.length === 0}>
        {statuses.map((status) => (
          <MenuItem
            key={status.key}
            onSelect={() => ops.setFolderStatus(folder.id, status.key, status.label)}
          >
            {status.key === folder.status ? `● ${status.label}` : status.label}
          </MenuItem>
        ))}
      </MenuSub>

      <MenuSub
        label="Archetype"
        // Empty until the user has made one; the app ships with none.
        disabled={archetypes.length === 0}
      >
        {archetypes.map((archetype) => (
          <MenuItem
            key={archetype.id}
            onSelect={() => ops.applyArchetype(folder.id, archetype.id, archetype.name)}
          >
            {archetype.name}
          </MenuItem>
        ))}
      </MenuSub>
      {/* `FolderNode` doesn't carry which archetype (if any) this folder is
          on, so this is always offered rather than conditionally hidden —
          it is a harmless no-op on a folder with nothing to remove. */}
      <MenuItem onSelect={() => ops.removeArchetype(folder.id)}>Remove archetype</MenuItem>

      <MenuItem
        onSelect={() => ops.setFolderFavorite(folder.id, folder.title, !folder.favorite)}
      >
        {folder.favorite ? "Unpin from the top" : "Pin to the top"}
      </MenuItem>
      <MenuItem onSelect={() => ops.setFolderCover(folder.id, null)}>
        Clear cover
      </MenuItem>

      <MenuSeparator />

      <MenuItem onSelect={() => ops.revealFolder(folder.id)}>
        Reveal in Explorer
      </MenuItem>

      <MenuSeparator />

      <MenuItem variant="destructive" onSelect={() => dialogs.deleteFolder(folder)}>
        Delete…
      </MenuItem>
    </>
  );
}

/** The same menu opened from a surface where no folder is under the pointer —
 *  the empty part of the navigation panel. */
export function FolderTreeBackgroundMenu({ root }: { root: FolderNode | null }) {
  const dialogs = useDialogs();
  return (
    <>
      <MenuLabel>Folders</MenuLabel>
      <MenuItem onSelect={() => dialogs.newFolder(root)}>
        New folder at the top level
      </MenuItem>
    </>
  );
}
