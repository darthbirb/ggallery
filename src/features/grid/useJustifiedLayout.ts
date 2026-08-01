import { useEffect, useMemo, useRef, useState } from "react";

import type { GridItem } from "../../lib/types";
import type { LayoutRequest, LayoutResult } from "./layoutWorker";

/** Default aspect for anything not probed yet, or probed and found square. */
const FALLBACK_ASPECT = 1;

/**
 * One layout worker for the life of the page, created lazily on first use.
 *
 * Component effects are what React's StrictMode double-invokes in
 * development — mount, cleanup, mount again, synchronously — so creating and
 * terminating a Worker there means doing exactly that twice in a row on
 * every mount. It was not the cause of the M1.6 blank-grid-in-dev defect
 * (that turned out to be a stale `requestAnimationFrame` handle in
 * `Grid.tsx`), but it is still needless churn for no benefit: the effect
 * below only ever needs to add and remove a listener, which is cheap and
 * idempotent regardless of how many times StrictMode runs it.
 */
let sharedWorker: Worker | null = null;
function layoutWorker(): Worker {
  if (!sharedWorker) {
    sharedWorker = new Worker(new URL("./layoutWorker.ts", import.meta.url), {
      type: "module",
    });
  }
  return sharedWorker;
}

export function useJustifiedLayout(
  items: GridItem[],
  containerWidth: number,
  targetHeight: number,
  gap: number,
): LayoutResult | null {
  const requestId = useRef(0);
  const [layout, setLayout] = useState<LayoutResult | null>(null);

  const aspects = useMemo(() => {
    const out = new Float32Array(items.length);
    for (let i = 0; i < items.length; i += 1) {
      const { w, h } = items[i];
      out[i] = w && h && h > 0 ? w / h : FALLBACK_ASPECT;
    }
    return out;
  }, [items]);

  // Only a listener is added and removed here — cheap and idempotent, so
  // StrictMode double-invoking it is harmless.
  useEffect(() => {
    const worker = layoutWorker();
    const onMessage = (event: MessageEvent<LayoutResult>) => {
      // A relayout in flight when another is requested is stale; dropping it
      // keeps a fast slider drag from painting layouts out of order.
      if (event.data.id !== requestId.current) return;
      setLayout(event.data);
    };
    worker.addEventListener("message", onMessage);
    return () => worker.removeEventListener("message", onMessage);
  }, []);

  useEffect(() => {
    if (containerWidth <= 0) return;

    requestId.current += 1;
    const request: LayoutRequest = {
      id: requestId.current,
      aspects,
      containerWidth,
      targetHeight,
      gap,
    };
    // Sent by copy: the aspects array belongs to the main thread and is reused
    // for every subsequent relayout.
    layoutWorker().postMessage(request);
  }, [aspects, containerWidth, targetHeight, gap]);

  return layout;
}

/** Index of the last row whose top is at or above `y`. */
export function rowAt(rowTops: Float32Array, rows: number, y: number): number {
  let low = 0;
  let high = rows - 1;
  let found = 0;
  while (low <= high) {
    const mid = (low + high) >> 1;
    if (rowTops[mid] <= y) {
      found = mid;
      low = mid + 1;
    } else {
      high = mid - 1;
    }
  }
  return found;
}
