/**
 * The menu look, shared by shadcn/ui's dropdown-menu and context-menu.
 *
 * Radix ships two primitive families with an identical item API, and shadcn
 * ships two near-identical style files for them. GGallery opens the same menu
 * definition either way — right-click on a folder row, or click the header's
 * button — so the classes live here once and `components/Menu.tsx` picks the
 * family. One definition, one look, no chance of the two drifting.
 *
 * Sized to decision 25: 32px rows, 14px labels, 12px mono for the shortcut
 * column, and enough horizontal padding that a menu does not read as a list
 * of links.
 */

import { cn } from "../../lib/utils";

/**
 * The floating surface every menu, submenu and popover uses.
 *
 * `outline-none` here is one of the two places it is correct: Radix focuses
 * the surface itself when a menu opens, and a ring around the whole panel is
 * not what "where am I" should mean. Items below suppress it for the same
 * reason — `data-highlighted` already says which row is live, and it follows
 * the keyboard and the mouse alike, so a second mark would be two answers to
 * one question. Nothing else in the app opts out of the focus ring.
 */
export const menuSurface = cn(
  "surface-in z-50 min-w-[212px] overflow-hidden rounded-[6px] border border-line bg-panel p-1 outline-none",
  "shadow-[0_16px_40px_rgba(0,0,0,0.55)]",
);

export const menuItem = cn(
  "relative flex h-8 cursor-pointer select-none items-center gap-2 rounded-[4px] px-2.5 text-[14px] outline-none",
  "data-[highlighted]:bg-hover data-[highlighted]:text-fg",
  "data-[disabled]:pointer-events-none data-[disabled]:opacity-35",
);

export const menuLabel = cn(
  "px-2.5 pb-1 pt-2 font-mono text-[12px] uppercase tracking-[0.1em] text-fg-dim",
);

export const menuSeparator = "-mx-1 my-1 h-px bg-line-soft";

/** Right-aligned keyboard equivalent. Never the only path to the action —
 *  locked decision 23. */
export const menuShortcut =
  "ml-auto pl-4 font-mono text-[12px] tabular-nums text-fg-dim";
