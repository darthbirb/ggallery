/**
 * Justified row layout for the whole library, computed once, off the main
 * thread.
 *
 * Validated in M0: a full 100k-item layout takes 7–18ms, so there is no need
 * for incremental or partial relayout at this scale. It stays in a worker
 * anyway — that decouples relayout from image decode and React commits on the
 * main thread, and costs nothing.
 */

export interface LayoutRequest {
  id: number;
  /** width / height per item, in display order. */
  aspects: Float32Array;
  containerWidth: number;
  targetHeight: number;
  gap: number;
}

export interface LayoutResult {
  id: number;
  rows: number;
  totalHeight: number;
  /** Ascending, length `rows + 1`; the last entry is `totalHeight`. */
  rowTops: Float32Array;
  rowHeights: Float32Array;
  rowStart: Uint32Array;
  rowLength: Uint32Array;
  itemLeft: Float32Array;
  itemWidth: Float32Array;
}

/** A row of one panorama should not become a wall. */
const MAX_ROW_SCALE = 2.4;
const MIN_ASPECT = 0.08;

export function computeLayout(request: LayoutRequest): LayoutResult {
  const { aspects, containerWidth, targetHeight, gap } = request;
  const count = aspects.length;

  const itemLeft = new Float32Array(count);
  const itemWidth = new Float32Array(count);
  const tops: number[] = [];
  const heights: number[] = [];
  const starts: number[] = [];
  const lengths: number[] = [];

  let y = 0;
  let index = 0;

  while (index < count && containerWidth > 0) {
    const start = index;
    let sum = 0;
    let width = 0;

    // Greedy: take items until the row, laid out at the target height, is at
    // least as wide as the container.
    while (index < count) {
      sum += Math.max(aspects[index], MIN_ASPECT);
      index += 1;
      width = sum * targetHeight + gap * (index - start - 1);
      if (width >= containerWidth) break;
    }

    const length = index - start;
    const available = containerWidth - gap * (length - 1);
    // The final row keeps the target height rather than being stretched to
    // fill; a last row of two photos blown up to full width looks like a bug.
    const incomplete = index >= count && width < containerWidth;
    const height = incomplete
      ? targetHeight
      : Math.min(available / sum, targetHeight * MAX_ROW_SCALE);

    let x = 0;
    for (let k = start; k < index; k += 1) {
      const w = Math.max(aspects[k], MIN_ASPECT) * height;
      itemLeft[k] = x;
      itemWidth[k] = w;
      x += w + gap;
    }

    tops.push(y);
    heights.push(height);
    starts.push(start);
    lengths.push(length);
    y += height + gap;
  }

  const rows = tops.length;
  const totalHeight = rows > 0 ? y - gap : 0;

  const rowTops = new Float32Array(rows + 1);
  rowTops.set(tops);
  rowTops[rows] = totalHeight;

  return {
    id: request.id,
    rows,
    totalHeight,
    rowTops,
    rowHeights: Float32Array.from(heights),
    rowStart: Uint32Array.from(starts),
    rowLength: Uint32Array.from(lengths),
    itemLeft,
    itemWidth,
  };
}

const worker = self as unknown as Worker;

worker.onmessage = (event: MessageEvent<LayoutRequest>) => {
  const result = computeLayout(event.data);
  // Typed arrays go back by transfer, not by copy.
  worker.postMessage(result, [
    result.rowTops.buffer,
    result.rowHeights.buffer,
    result.rowStart.buffer,
    result.rowLength.buffer,
    result.itemLeft.buffer,
    result.itemWidth.buffer,
  ]);
};
