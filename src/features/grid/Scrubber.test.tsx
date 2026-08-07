/**
 * The scrubber, and what SPEC.md §2 asks of it: no permanent column of
 * year and month labels, and — since M2.5a.2 — no date at all, in any state.
 * Two passes tried to make a date readable next to a thumb held for under a
 * second; the position is the information.
 *
 * These are interaction tests, not appearance tests — "is there text down the
 * strip" is a behavioural question, and it is the one that regressed once
 * already (M2.5a.1 brought a date back for the drag).
 */

import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { Scrubber } from "./Scrubber";

function renderScrubber() {
  const onJump = vi.fn();
  const result = render(<Scrubber onJump={onJump} />);
  const strip = screen.getByLabelText("Timeline");

  // jsdom has no layout, so the strip has to be told how tall it is for the
  // pointer maths to mean anything.
  strip.getBoundingClientRect = () =>
    ({ top: 0, left: 0, height: 300, width: 16 }) as DOMRect;

  return { ...result, strip, onJump };
}

/** The component coalesces jumps into one animation frame; flush it. */
async function flushFrame() {
  await act(async () => {
    await new Promise((resolve) => requestAnimationFrame(() => resolve(null)));
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe("the scrubber", () => {
  it("draws no year, month or date column at rest", () => {
    const { strip } = renderScrubber();
    expect(strip.textContent).toBe("");
  });

  it("jumps on pointer down and draws no date while dragging", async () => {
    const { strip, onJump } = renderScrubber();

    fireEvent.pointerDown(strip, { clientY: 250, pointerId: 1, button: 0 });
    await flushFrame();

    expect(onJump).toHaveBeenCalledWith(250 / 300);
    expect(strip.textContent).toBe("");
  });

  it("keeps jumping while the drag continues, coalesced to one call per frame", async () => {
    const { strip, onJump } = renderScrubber();

    fireEvent.pointerDown(strip, { clientY: 20, pointerId: 1, button: 0 });
    fireEvent.pointerMove(strip, { clientY: 120, pointerId: 1 });
    await flushFrame();

    expect(onJump).toHaveBeenLastCalledWith(120 / 300);

    fireEvent.pointerUp(strip, { clientY: 120, pointerId: 1 });
    expect(strip.textContent).toBe("");
  });
});
