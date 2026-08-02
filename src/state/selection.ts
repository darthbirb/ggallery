import { useCallback, useMemo, useState } from "react";

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
  /**
   * The item the pane is previewing — the last one clicked, which is not the
   * same question as "what is selected". A range of forty items has one
   * current item, and stepping through the filter with the chevrons moves it
   * without collapsing the selection to one.
   */
  current: number | null;
  isSelected: (id: number) => boolean;
  /** Plain click replaces the selection with just this item. Ctrl/Cmd-click
   *  toggles it. Shift-click extends a range from the last clicked item, in
   *  the current item order — the same set the grid is showing. Drag-marquee
   *  selection arrives with direct manipulation in M2.5b. */
  click: (id: number, modifiers: ClickModifiers) => void;
  /** Make one item current and selected — used by the preview's chevrons and
   *  the filmstrip. */
  focus: (id: number) => void;
  /** Move the current item through the grid's order. Returns the id landed
   *  on, or null at either end. */
  step: (delta: number) => number | null;
  selectAll: () => void;
  invert: () => void;
  clear: () => void;
}

export function useSelection(items: GridItem[]): SelectionController {
  const [selected, setSelected] = useState<Set<number>>(() => new Set());
  const [current, setCurrent] = useState<number | null>(null);

  const index = useMemo(() => {
    const map = new Map<number, number>();
    items.forEach((item, at) => map.set(item.id, at));
    return map;
  }, [items]);

  const click = useCallback(
    (id: number, modifiers: ClickModifiers) => {
      if (modifiers.shiftKey && current !== null) {
        const from = index.get(current);
        const to = index.get(id);
        if (from === undefined || to === undefined) {
          setSelected(new Set([id]));
          setCurrent(id);
          return;
        }
        const [start, end] = from < to ? [from, to] : [to, from];
        setSelected(new Set(items.slice(start, end + 1).map((item) => item.id)));
        setCurrent(id);
        return;
      }

      if (modifiers.ctrlKey || modifiers.metaKey) {
        setSelected((existing) => {
          const next = new Set(existing);
          if (next.has(id)) next.delete(id);
          else next.add(id);
          return next;
        });
        setCurrent(id);
        return;
      }

      setSelected(new Set([id]));
      setCurrent(id);
    },
    [current, index, items],
  );

  const focus = useCallback((id: number) => {
    setSelected(new Set([id]));
    setCurrent(id);
  }, []);

  const step = useCallback(
    (delta: number) => {
      if (items.length === 0) return null;
      const at = current === null ? -1 : (index.get(current) ?? -1);
      const next = at === -1 ? (delta > 0 ? 0 : items.length - 1) : at + delta;
      if (next < 0 || next >= items.length) return null;
      const id = items[next].id;
      focus(id);
      return id;
    },
    [items, index, current, focus],
  );

  const selectAll = useCallback(() => {
    setSelected(new Set(items.map((item) => item.id)));
  }, [items]);

  const invert = useCallback(() => {
    setSelected((existing) => {
      const next = new Set<number>();
      for (const item of items) {
        if (!existing.has(item.id)) next.add(item.id);
      }
      return next;
    });
  }, [items]);

  const clear = useCallback(() => {
    setSelected(new Set());
    setCurrent(null);
  }, []);

  const isSelected = useCallback((id: number) => selected.has(id), [selected]);

  return {
    selected,
    count: selected.size,
    current,
    isSelected,
    click,
    focus,
    step,
    selectAll,
    invert,
    clear,
  };
}
