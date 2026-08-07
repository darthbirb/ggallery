/**
 * Spring-loading (SPEC.md §*Drops*): hovering a drop target during a drag
 * opens it after a dwell, but plain mouse hover — no drag in progress —
 * must never trigger it.
 */

import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { DndProvider, useDnd, useSpringLoad } from "./dnd";

function Probe({ onTrigger }: { onTrigger: () => void }) {
  const { startDrag } = useDnd();
  const springLoad = useSpringLoad(onTrigger, 700);
  return (
    <>
      <button onClick={() => startDrag({ kind: "items", itemIds: [1] })}>start drag</button>
      <div
        data-testid="target"
        onDragEnter={springLoad.onDragEnter}
        onDragLeave={springLoad.onDragLeave}
        onDrop={springLoad.onDrop}
      />
    </>
  );
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe("useSpringLoad", () => {
  it("opens the target after the dwell, once a drag is in progress", () => {
    const onTrigger = vi.fn();
    render(
      <DndProvider>
        <Probe onTrigger={onTrigger} />
      </DndProvider>,
    );

    act(() => fireEvent.click(screen.getByText("start drag")));
    fireEvent.dragEnter(screen.getByTestId("target"));
    expect(onTrigger).not.toHaveBeenCalled();

    act(() => vi.advanceTimersByTime(699));
    expect(onTrigger).not.toHaveBeenCalled();
    act(() => vi.advanceTimersByTime(1));
    expect(onTrigger).toHaveBeenCalledTimes(1);
  });

  it("never triggers from plain hover with no drag in progress", () => {
    const onTrigger = vi.fn();
    render(
      <DndProvider>
        <Probe onTrigger={onTrigger} />
      </DndProvider>,
    );

    fireEvent.dragEnter(screen.getByTestId("target"));
    act(() => vi.advanceTimersByTime(2000));
    expect(onTrigger).not.toHaveBeenCalled();
  });

  it("cancels if the pointer leaves before the dwell completes", () => {
    const onTrigger = vi.fn();
    render(
      <DndProvider>
        <Probe onTrigger={onTrigger} />
      </DndProvider>,
    );

    act(() => fireEvent.click(screen.getByText("start drag")));
    fireEvent.dragEnter(screen.getByTestId("target"));
    act(() => vi.advanceTimersByTime(400));
    fireEvent.dragLeave(screen.getByTestId("target"));
    act(() => vi.advanceTimersByTime(1000));
    expect(onTrigger).not.toHaveBeenCalled();
  });

  it("does not double-fire when a nested child fires its own enter and leave first", () => {
    // The classic dragenter/dragleave flicker: a child element inside the
    // target fires its own enter before the target's leave, which a naive
    // boolean would misread as "left the target". The enter counter is what
    // keeps this from cancelling the dwell early.
    const onTrigger = vi.fn();
    render(
      <DndProvider>
        <Probe onTrigger={onTrigger} />
      </DndProvider>,
    );

    act(() => fireEvent.click(screen.getByText("start drag")));
    const target = screen.getByTestId("target");
    fireEvent.dragEnter(target); // enters the target
    fireEvent.dragEnter(target); // enters a child
    fireEvent.dragLeave(target); // leaves the child, back over the target
    act(() => vi.advanceTimersByTime(700));
    expect(onTrigger).toHaveBeenCalledTimes(1);
  });
});
