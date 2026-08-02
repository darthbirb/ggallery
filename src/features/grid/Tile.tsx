/**
 * Tiles are recycled DOM nodes, not React components — deliberately.
 *
 * M0 measured mount/unmount churn during a fling generating enough garbage
 * (a fresh `<img>` and decode object per tile) to force a major GC, producing
 * 88–105ms frames and failing the "no blank frame held over 100ms" target.
 * See docs/ENGINEERING-NOTES.md §1. So: a fixed pool of nodes is created once
 * and repositioned as the visible range moves, `<img>` elements are reused by
 * setting `src`, and every piece of decoration (duration, favourite badge,
 * selection ring, scrub strip) is a permanent child toggled by class rather
 * than a conditionally rendered child.
 *
 * The file keeps its `Tile.tsx` name from docs/STRUCTURE.md; it exports the
 * pool that owns tile DOM rather than a component.
 */

import { formatDuration } from "../../lib/format";
import { assetUrl } from "../../lib/ipc";
import type { GridItem } from "../../lib/types";
import type { ClickModifiers } from "../../state/selection";
import type { LayoutResult } from "./layoutWorker";

/** Frames in a scrub strip. Must match `media::sprites::FRAMES`. */
const SPRITE_FRAMES = 10;

interface TileNode {
  root: HTMLDivElement;
  img: HTMLImageElement;
  scrub: HTMLDivElement;
  duration: HTMLSpanElement;
  /** Item currently displayed, or -1 when parked in the free list. */
  itemId: number;
  itemIndex: number;
  kind: string;
  thumb: string;
  transform: string;
  width: number;
  height: number;
  failed: boolean;
  mark: number;
}

export interface TilePoolOptions {
  container: HTMLElement;
  onSelect: (id: number, modifiers: ClickModifiers) => void;
  /** Right-click. The pool reports where and on what; the grid opens the
   *  menu, because a recycled node cannot own a React portal. */
  onContext: (id: number, x: number, y: number) => void;
  /** Double-click — show this item in the pane. */
  onActivate: (id: number) => void;
}

export class TilePool {
  private readonly container: HTMLElement;
  private readonly onSelect: (id: number, modifiers: ClickModifiers) => void;
  private readonly onContext: (id: number, x: number, y: number) => void;
  private readonly onActivate: (id: number) => void;
  private readonly nodes: TileNode[] = [];
  private readonly free: TileNode[] = [];
  private readonly active = new Map<number, TileNode>();

  private items: GridItem[] = [];
  private thumbsDir = "";
  private spritesDir = "";
  private selected: Set<number> = new Set();
  /** The item the pane is previewing — one, and not necessarily in the
   *  selection. Marked differently so both can be read at once. */
  private current: number | null = null;
  private mark = 0;

  constructor(options: TilePoolOptions) {
    this.container = options.container;
    this.onSelect = options.onSelect;
    this.onContext = options.onContext;
    this.onActivate = options.onActivate;
  }

  setItems(items: GridItem[], thumbsDir: string, spritesDir: string): void {
    this.items = items;
    this.thumbsDir = thumbsDir;
    this.spritesDir = spritesDir;
    // Same node, different library or different query: force a repopulate.
    for (const node of this.nodes) node.itemId = -1;
  }

  setSelected(ids: Set<number>): void {
    this.selected = ids;
    for (const node of this.active.values()) {
      node.root.classList.toggle("is-selected", ids.has(node.itemId));
    }
  }

  setCurrent(id: number | null): void {
    this.current = id;
    for (const node of this.active.values()) {
      node.root.classList.toggle("is-current", node.itemId === id);
    }
  }

  /** Reposition and repopulate for a visible row range. Allocates only when
   *  the visible range has grown beyond every node built so far. */
  sync(startRow: number, endRow: number, layout: LayoutResult): void {
    this.mark += 1;

    for (let row = startRow; row <= endRow && row < layout.rows; row += 1) {
      const top = layout.rowTops[row];
      const height = layout.rowHeights[row];
      const start = layout.rowStart[row];
      const end = start + layout.rowLength[row];

      for (let index = start; index < end && index < this.items.length; index += 1) {
        const node = this.acquire(index);
        this.place(node, index, layout.itemLeft[index], top, layout.itemWidth[index], height);
        node.mark = this.mark;
      }
    }

    for (const [index, node] of this.active) {
      if (node.mark !== this.mark) this.release(index, node);
    }
  }

  /** Re-request thumbnails that were not on disk yet. Indexing generates them
   *  while the grid is already showing their items. */
  retryMissing(): void {
    for (const node of this.active.values()) {
      if (!node.failed) continue;
      node.failed = false;
      node.img.src = assetUrl(this.thumbsDir, node.thumb);
    }
  }

  destroy(): void {
    for (const node of this.nodes) node.root.remove();
    this.nodes.length = 0;
    this.free.length = 0;
    this.active.clear();
  }

  private acquire(index: number): TileNode {
    const existing = this.active.get(index);
    if (existing) return existing;

    const node = this.free.pop() ?? this.create();
    node.root.style.display = "";
    node.itemIndex = index;
    this.active.set(index, node);
    return node;
  }

  private release(index: number, node: TileNode): void {
    node.root.style.display = "none";
    node.root.classList.remove("is-scrubbing");
    this.active.delete(index);
    this.free.push(node);
  }

  private place(
    node: TileNode,
    index: number,
    x: number,
    y: number,
    width: number,
    height: number,
  ): void {
    const item = this.items[index];

    const transform = `translate3d(${Math.round(x)}px, ${Math.round(y)}px, 0)`;
    if (node.transform !== transform) {
      node.root.style.transform = transform;
      node.transform = transform;
    }
    const w = Math.round(width);
    if (node.width !== w) {
      node.root.style.width = `${w}px`;
      node.width = w;
    }
    const h = Math.round(height);
    if (node.height !== h) {
      node.root.style.height = `${h}px`;
      node.height = h;
    }

    if (node.itemId === item.id) return;

    node.itemId = item.id;
    node.itemIndex = index;
    node.kind = item.kind;
    node.thumb = item.thumb;
    node.failed = false;

    node.img.classList.add("is-empty");
    node.img.src = assetUrl(this.thumbsDir, item.thumb);

    node.root.classList.toggle("is-video", item.kind === "video");
    node.root.classList.toggle("is-favorite", item.favorite);
    node.root.classList.toggle("is-selected", this.selected.has(item.id));
    node.root.classList.toggle("is-current", this.current === item.id);
    node.root.classList.remove("is-scrubbing");
    node.scrub.style.backgroundImage = "";
    node.duration.textContent =
      item.kind === "video" ? formatDuration(item.durationMs) : "";
  }

  private create(): TileNode {
    const root = document.createElement("div");
    root.className = "tile";

    const img = document.createElement("img");
    img.className = "tile-img is-empty";
    img.draggable = false;
    img.decoding = "async";

    const scrub = document.createElement("div");
    scrub.className = "tile-scrub";

    const duration = document.createElement("span");
    duration.className = "tile-badge tile-dur";

    const favorite = document.createElement("span");
    favorite.className = "tile-badge tile-fav";
    favorite.textContent = "★";

    root.append(img, scrub, duration, favorite);

    const node: TileNode = {
      root,
      img,
      scrub,
      duration,
      itemId: -1,
      itemIndex: -1,
      kind: "other",
      thumb: "",
      transform: "",
      width: -1,
      height: -1,
      failed: false,
      mark: -1,
    };

    // Listeners are attached once per node, for the life of the window. Nodes
    // are never destroyed, so nothing here is ever removed or re-created.
    img.addEventListener("load", () => img.classList.remove("is-empty"));
    img.addEventListener("error", () => {
      node.failed = true;
    });
    root.addEventListener("click", (event) => {
      if (node.itemId >= 0) {
        this.onSelect(node.itemId, {
          ctrlKey: event.ctrlKey,
          metaKey: event.metaKey,
          shiftKey: event.shiftKey,
        });
      }
    });
    root.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      // React attaches its listeners at the root container, so stopping
      // propagation here is what keeps the grid's own background menu from
      // firing straight after this one and replacing it.
      event.stopPropagation();
      if (node.itemId >= 0) this.onContext(node.itemId, event.clientX, event.clientY);
    });
    root.addEventListener("dblclick", () => {
      if (node.itemId >= 0) this.onActivate(node.itemId);
    });
    root.addEventListener("mouseenter", () => {
      if (node.kind !== "video") return;
      if (!node.scrub.style.backgroundImage) {
        node.scrub.style.backgroundImage = `url("${assetUrl(this.spritesDir, node.thumb)}")`;
      }
      root.classList.add("is-scrubbing");
    });
    root.addEventListener("mousemove", (event) => {
      if (node.kind !== "video") return;
      const rect = root.getBoundingClientRect();
      const fraction = Math.min(
        0.9999,
        Math.max(0, (event.clientX - rect.left) / Math.max(rect.width, 1)),
      );
      const frame = Math.floor(fraction * SPRITE_FRAMES);
      node.scrub.style.backgroundPosition = `${(frame / (SPRITE_FRAMES - 1)) * 100}% 0`;
    });
    root.addEventListener("mouseleave", () => {
      root.classList.remove("is-scrubbing");
    });

    this.container.appendChild(root);
    this.nodes.push(node);
    return node;
  }
}
