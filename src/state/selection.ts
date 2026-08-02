import { useCallback, useState } from "react";

import type { GridItem } from "../lib/types";

/** The subset of a mouse event `click` actually needs — kept narrow so
 *  callers (a real `MouseEvent`, a synthetic React one) don't have to match
 *  a bigger shape than this. */
export interface ClickModifiers {
  ctrlKey: boolean;
  metaKey: boolean;
  shiftKey: boolean;
}

export interface SelectionController {
  selected: Set<number>;
  count: number;
  isSelected: (id: number) => boolean;
  /** Plain click replaces the selection with just this item. Ctrl/Cmd-click
   *  toggles it. Shift-click extends a range from the last clicked item, in
   *  the current item order — the same set the grid is showing. Drag-marquee
   *  selection is M2.5's (docs/DESIGN.md "Selection"); this covers enough to
   *  exercise every operation that acts on a selection. */
  click: (id: number, modifiers: ClickModifiers) => void;
  selectAll: () => void;
  invert: () => void;
  clear: () => void;
}

export function useSelection(items: GridItem[]): SelectionController {
  const [selected, setSelected] = useState<Set<number>>(() => new Set());
  const [anchor, setAnchor] = useState<number | null>(null);

  const click = useCallback(
    (id: number, modifiers: ClickModifiers) => {
      if (modifiers.shiftKey && anchor !== null) {
        const from = items.findIndex((item) => item.id === anchor);
        const to = items.findIndex((item) => item.id === id);
        if (from === -1 || to === -1) {
          setSelected(new Set([id]));
          setAnchor(id);
          return;
        }
        const [start, end] = from < to ? [from, to] : [to, from];
        setSelected(new Set(items.slice(start, end + 1).map((item) => item.id)));
        return;
      }

      if (modifiers.ctrlKey || modifiers.metaKey) {
        setSelected((current) => {
          const next = new Set(current);
          if (next.has(id)) next.delete(id);
          else next.add(id);
          return next;
        });
        setAnchor(id);
        return;
      }

      setSelected(new Set([id]));
      setAnchor(id);
    },
    [anchor, items],
  );

  const selectAll = useCallback(() => {
    setSelected(new Set(items.map((item) => item.id)));
  }, [items]);

  const invert = useCallback(() => {
    setSelected((current) => {
      const next = new Set<number>();
      for (const item of items) {
        if (!current.has(item.id)) next.add(item.id);
      }
      return next;
    });
  }, [items]);

  const clear = useCallback(() => {
    setSelected(new Set());
    setAnchor(null);
  }, []);

  const isSelected = useCallback((id: number) => selected.has(id), [selected]);

  return { selected, count: selected.size, isSelected, click, selectAll, invert, clear };
}
