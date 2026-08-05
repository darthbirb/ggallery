/**
 * Test setup. Interaction tests, not appearance tests — see PLAN.md §M2.5
 * "Build notes": does picking an archetype call the right command, does
 * editing a label persist, does adding a flag update the tag set. That is the
 * class of bug M2 hit (an archetype dropdown that focused the notes field
 * instead of registering the selection), and it is invisible to Rust tests
 * and to `tsc`.
 *
 * `lib/ipc` is mocked per test file, so nothing here ever reaches Tauri.
 */

import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach, vi } from "vitest";

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});

// jsdom implements neither pointer capture nor layout, and Radix's menus and
// sliders use both. These are the standard shims; without them every menu
// test fails on the primitive rather than on the app.
if (!Element.prototype.hasPointerCapture) {
  Element.prototype.hasPointerCapture = () => false;
  Element.prototype.setPointerCapture = () => {};
  Element.prototype.releasePointerCapture = () => {};
}
if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}

if (!("ResizeObserver" in globalThis)) {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}

// jsdom implements no Worker at all, and `Grid` (used directly by M2.5b's
// GridMode) creates one lazily on mount for its justified-layout math. The
// interaction tests here never assert on pixel layout, only on what renders
// regardless of it (an empty state, a tile count) — so a Worker that never
// actually posts a message back is enough to let `Grid` mount without
// throwing, same reasoning as every other shim in this file.
if (!("Worker" in globalThis)) {
  globalThis.Worker = class {
    addEventListener() {}
    removeEventListener() {}
    postMessage() {}
    terminate() {}
  } as unknown as typeof Worker;
}

if (!("DOMRect" in globalThis)) {
  globalThis.DOMRect = class {
    constructor(
      public x = 0,
      public y = 0,
      public width = 0,
      public height = 0,
    ) {}
    top = 0;
    left = 0;
    right = 0;
    bottom = 0;
    toJSON() {
      return this;
    }
    static fromRect() {
      return new globalThis.DOMRect();
    }
  } as unknown as typeof DOMRect;
}
