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

import type { GridItem } from "../../lib/types";
import type { PaneMode } from "../../state/ui";
import { PreviewMode, type PreviewSlot } from "./PreviewMode";

export interface PaneProps {
  mode: PaneMode;
  onClose: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;

  slots: PreviewSlot[];
  items: GridItem[];
  thumbsDir: string;
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
  detailsExpanded: boolean;
  onDetailsExpandedChange: (expanded: boolean) => void;
  filmstripHeight: number;
  onFilmstripHeightChange: (height: number) => void;
  onResetFilmstripHeight: () => void;
  refreshToken: number;
}

export function Pane({ mode, ...rest }: PaneProps) {
  // Grid and Folders are M2.5b's; each will render its own `PaneFrame`.
  if (mode !== "preview") return null;
  return <PreviewMode {...rest} />;
}
