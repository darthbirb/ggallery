/**
 * The pane: the right half of the split, and the single most reused surface
 * in the app.
 *
 * Drag-resizable (the handle is the shell's, since it sits between the two
 * halves), fully closable. **There is no theatre view** — full-window is the
 * pane maximised, one control and one state.
 *
 * This file only picks the mode. Everything the modes have in common is
 * `PaneFrame`, which each of them renders for itself — that is what lets a
 * mode own its header without this file knowing what any of them put there.
 */

import { PanelRightOpen } from "lucide-react";

import { Tooltip } from "../../components/Tooltip";
import { IconButton } from "../../components/ui/button";
import type { FolderNode, GridItem } from "../../lib/types";
import { AVAILABLE_PANE_MODES, PANE_MODES, type PaneMode } from "../../state/ui";
import { FoldersMode } from "./FoldersMode";
import { GridMode } from "./GridMode";
import { PANE_MODE_ICONS } from "./modeIcons";
import { PreviewMode, type PreviewSlot } from "./PreviewMode";

export interface PaneProps {
  mode: PaneMode;
  onModeChange: (mode: PaneMode) => void;
  onClose: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;

  slots: PreviewSlot[];
  items: GridItem[];
  thumbsDir: string;
  spritesDir: string;
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
  detailsExpanded: boolean;
  onDetailsExpandedChange: (expanded: boolean) => void;
  filmstripHeight: number;
  onFilmstripHeightChange: (height: number) => void;
  onResetFilmstripHeight: () => void;
  refreshToken: number;

  /** Grid and Folders modes need the whole tree — Preview does not. */
  folders: FolderNode[];
  /** Folders mode's double-click — the one gesture that moves the main
   *  grid, per SPEC.md §2 *Folders mode*. */
  onOpenInMain: (folder: FolderNode) => void;
}

export function Pane({ mode, folders, onOpenInMain, spritesDir, ...rest }: PaneProps) {
  if (mode === "grid") {
    return (
      <GridMode
        mode={mode}
        onModeChange={rest.onModeChange}
        onClose={rest.onClose}
        maximised={rest.maximised}
        onMaximisedChange={rest.onMaximisedChange}
        folders={folders}
        refreshToken={rest.refreshToken}
        thumbsDir={rest.thumbsDir}
        spritesDir={spritesDir}
        onPreview={rest.onPick}
      />
    );
  }
  if (mode === "folders") {
    return (
      <FoldersMode
        mode={mode}
        onModeChange={rest.onModeChange}
        onClose={rest.onClose}
        maximised={rest.maximised}
        onMaximisedChange={rest.onMaximisedChange}
        folders={folders}
        refreshToken={rest.refreshToken}
        thumbsDir={rest.thumbsDir}
        onOpenInMain={onOpenInMain}
      />
    );
  }
  return <PreviewMode mode={mode} {...rest} />;
}

/**
 * Closed, the pane folds to a strip that mirrors the navigation panel's own
 * fold exactly: a bordered 44px header holding the button that reopens it,
 * then the mode icons below — not a separate "Open pane" control, because a
 * control that exists only while a panel is closed is chrome that has to
 * live somewhere, and the panel's own edge is where it belongs. Preview is
 * the only mode that does anything yet; Grid and Folders sit here inert,
 * like the nav's own placeholder pair, rather than disappearing.
 */
export function PaneStrip({
  mode,
  onOpen,
}: {
  mode: PaneMode;
  onOpen: (mode: PaneMode) => void;
}) {
  return (
    <nav
      aria-label="Pane"
      className="flex h-full w-11 shrink-0 flex-col border-l border-line bg-panel"
    >
      <div className="flex h-11 shrink-0 items-center justify-center border-b border-line-soft">
        <Tooltip label="Show The Pane" side="left">
          <IconButton aria-label="Show The Pane" onClick={() => onOpen(mode)}>
            <PanelRightOpen />
          </IconButton>
        </Tooltip>
      </div>

      <div className="flex flex-col items-center gap-2 pt-2">
        {PANE_MODES.map((candidate) => {
          const Icon = PANE_MODE_ICONS[candidate.key];
          const built = AVAILABLE_PANE_MODES.some((option) => option.key === candidate.key);
          return (
            <Tooltip key={candidate.key} label={candidate.label} side="left">
              <IconButton
                aria-label={candidate.label}
                active={mode === candidate.key}
                onClick={built ? () => onOpen(candidate.key) : () => {}}
              >
                <Icon />
              </IconButton>
            </Tooltip>
          );
        })}
      </div>
    </nav>
  );
}
