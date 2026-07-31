import { useState } from "react";

/** Row heights the size control steps through, in pixels. */
export const TILE_SIZES = [96, 132, 180, 240, 320];

export interface UiState {
  tileHeight: number;
  setTileHeight: (height: number) => void;
}

export function useUi(): UiState {
  const [tileHeight, setTileHeight] = useState(TILE_SIZES[1]);
  return { tileHeight, setTileHeight };
}
