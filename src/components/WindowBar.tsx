/**
 * The window bar — decision 28. Ours, not Windows': native decorations are
 * off (`src-tauri/src/lib.rs`), so this replaces them entirely. Owns exactly
 * three things — the mark, the app name, and the window controls — plus a
 * drag region for everything between. Search joins it in M3.
 *
 * **Known cost, accepted deliberately:** Windows 11's Snap Layouts flyout
 * only appears over a *native* maximise button, which hit-testing it as
 * `HTMAXBUTTON` in Rust would be the only way to recover — not worth it for
 * one window on one monitor. Edge-drag resizing and edge-snap still work;
 * both are handled by Tauri below the decorations flag.
 *
 * Caption buttons are deliberately not the app's `IconButton` — they mimic
 * OS chrome the user already has muscle memory for (wide flat rectangles
 * flush to the top corners, not square with a border at rest), not an
 * ordinary control under decision 25's sizing scale.
 */

import { getCurrentWindow, type Window } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState, type ButtonHTMLAttributes } from "react";

import { cn } from "../lib/utils";
import { Mark } from "./Mark";

/** `getCurrentWindow()` dereferences `window.__TAURI_INTERNALS__` the moment
 *  it is called — unlike `invoke()`, which just rejects, this throws
 *  synchronously outside a real Tauri window. `npm run dev` opens this app
 *  in a plain browser tab (see CLAUDE.md, "Seeing what you built"), so the
 *  call has to be deferred until this component actually mounts, guarded by
 *  the same global the function itself reads — never at module scope, or
 *  every dev-mode preview that imports this file breaks before anything
 *  renders. */
function currentWindow(): Window | null {
  if (typeof window === "undefined" || !("__TAURI_INTERNALS__" in window)) return null;
  return getCurrentWindow();
}

export function WindowBar() {
  const [win] = useState(currentWindow);
  const [maximised, setMaximised] = useState(false);

  useEffect(() => {
    if (!win) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void (async () => {
      const initial = await win.isMaximized();
      if (!cancelled) setMaximised(initial);
      unlisten = await win.onResized(async () => {
        const next = await win.isMaximized();
        if (!cancelled) setMaximised(next);
      });
    })();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [win]);

  return (
    <header className="flex h-8 shrink-0 select-none items-stretch border-b border-line bg-panel">
      {/* Not a drag region: the mark and name are content, not chrome to
          grab. Double-clicking beside them (the spacer below) maximises. */}
      <div className="flex shrink-0 items-center gap-1.5 pl-2 pr-3">
        <Mark className="size-4" />
        <span className="text-[13px] font-semibold tracking-tight text-fg">GGallery</span>
      </div>

      <div
        data-tauri-drag-region
        onDoubleClick={() => void win?.toggleMaximize()}
        className="h-full flex-1"
      />

      <div className="flex shrink-0 items-stretch">
        <CaptionButton aria-label="Minimise" onClick={() => void win?.minimize()}>
          <Minus className="size-3.5" />
        </CaptionButton>
        <CaptionButton
          aria-label={maximised ? "Restore down" : "Maximise"}
          onClick={() => void win?.toggleMaximize()}
        >
          {maximised ? <Copy className="size-3.5" /> : <Square className="size-3" />}
        </CaptionButton>
        <CaptionButton aria-label="Close" close onClick={() => void win?.close()}>
          <X className="size-4" />
        </CaptionButton>
      </div>
    </header>
  );
}

function CaptionButton({
  children,
  close,
  className,
  ...rest
}: ButtonHTMLAttributes<HTMLButtonElement> & { close?: boolean }) {
  return (
    <button
      type="button"
      className={cn(
        "flex w-11 items-center justify-center text-fg-mid transition-colors duration-100",
        close ? "hover:bg-danger hover:text-white" : "hover:bg-hover hover:text-fg",
        className,
      )}
      {...rest}
    >
      {children}
    </button>
  );
}
