/**
 * Icons for the pane's three modes, shared by the header's switcher
 * (`PaneFrame`) and the closed pane's fold strip (`Pane.tsx`'s `PaneStrip`).
 *
 * A leaf module on purpose: `PaneFrame` cannot import from `Pane.tsx` (the
 * modes import `PaneFrame`, and `Pane` imports the modes — see the comment
 * there), so anything both need lives here instead.
 */

import { FolderTree, Image, LayoutGrid, type LucideIcon } from "lucide-react";

import type { PaneMode } from "../../state/ui";

export const PANE_MODE_ICONS: Record<PaneMode, LucideIcon> = {
  preview: Image,
  grid: LayoutGrid,
  folders: FolderTree,
};
