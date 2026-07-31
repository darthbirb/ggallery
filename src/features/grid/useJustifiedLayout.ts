import { useEffect, useMemo, useRef, useState } from "react";

import type { GridItem } from "../../lib/types";
import type { LayoutRequest, LayoutResult } from "./layoutWorker";

/** Default aspect for anything not probed yet, or probed and found square. */
const FALLBACK_ASPECT = 1;

export function useJustifiedLayout(
  items: GridItem[],
  containerWidth: number,
  targetHeight: number,
  gap: number,
): LayoutResult | null {
  const workerRef = useRef<Worker | null>(null);
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

  useEffect(() => {
    const worker = new Worker(new URL("./layoutWorker.ts", import.meta.url), {
      type: "module",
    });
    workerRef.current = worker;

    worker.onmessage = (event: MessageEvent<LayoutResult>) => {
      // A relayout in flight when another is requested is stale; dropping it
      // keeps a fast slider drag from painting layouts out of order.
      if (event.data.id !== requestId.current) return;
      setLayout(event.data);
    };

    return () => {
      worker.terminate();
      workerRef.current = null;
    };
  }, []);

  useEffect(() => {
    const worker = workerRef.current;
    if (!worker || containerWidth <= 0) return;

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
    worker.postMessage(request);
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
