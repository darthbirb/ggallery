import { useState } from "react";

/** Row heights the size control steps through, in pixels. */
export const TILE_SIZES = [96, 132, 180, 240, 320];

export interface UiState {
  tileHeight: number;
  setTileHeight: (height: number) => void;
  folderHeaderCollapsed: boolean;
  setFolderHeaderCollapsed: (collapsed: boolean) => void;
}

export function useUi(): UiState {
  const [tileHeight, setTileHeight] = useState(TILE_SIZES[1]);
  const [folderHeaderCollapsed, setFolderHeaderCollapsed] = useState(false);
  return {
    tileHeight,
    setTileHeight,
    folderHeaderCollapsed,
    setFolderHeaderCollapsed,
  };
}
