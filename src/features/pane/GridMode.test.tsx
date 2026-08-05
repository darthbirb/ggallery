/**
 * The pane's Grid mode — DESIGN.md §2 *Grid mode*: a second grid, scoped
 * anywhere in the library, that accepts drops and moves whatever lands on
 * it into whatever folder it is currently showing. Dropping onto it while
 * it shows Everything has nowhere real to file into, so it must refuse.
 */

import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, gridItem, renderWithProviders } from "../../test/harness";
import type { DragPayload } from "../../state/dnd";
import { useDnd } from "../../state/dnd";
import type { FolderNode } from "../../lib/types";
import { GridMode } from "./GridMode";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

function node(over: Partial<FolderNode>): FolderNode {
  return {
    id: 1,
    title: "trips",
    parentId: null,
    depth: 0,
    directCount: 0,
    totalCount: 0,
    status: "active",
    favorite: false,
    ...over,
  };
}

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.listItems.mockResolvedValue([]);
});

function DragStarter({ payload }: { payload: DragPayload }) {
  const { startDrag } = useDnd();
  return (
    <button type="button" onClick={() => startDrag(payload)}>
      start drag
    </button>
  );
}

function renderGridMode(folders: FolderNode[], extra: React.ReactNode = null) {
  return renderWithProviders(
    <>
      <GridMode
        mode="grid"
        onModeChange={vi.fn()}
        onClose={vi.fn()}
        maximised={false}
        onMaximisedChange={vi.fn()}
        folders={folders}
        refreshToken={0}
        thumbsDir="D:/thumbs"
        spritesDir="D:/sprites"
        onPreview={vi.fn()}
      />
      {extra}
    </>,
    { library: fakeLibrary({ folders }) },
  );
}

describe("scope", () => {
  it("shows Everything by default", async () => {
    renderGridMode([]);
    expect(await screen.findByText("Everything")).toBeInTheDocument();
    await waitFor(() => expect(mocked.listItems).toHaveBeenCalledWith(null, false, true));
  });

  it("can be pointed at a different folder through the picker", async () => {
    const folders = [node({ id: 1, title: "trips" })];
    mocked.listItems.mockResolvedValue([gridItem({ id: 7 })]);
    renderGridMode(folders);

    await userEvent.click(await screen.findByText("Everything"));
    await userEvent.click(await screen.findByRole("button", { name: /trips/ }));

    await waitFor(() => expect(mocked.listItems).toHaveBeenCalledWith(1, false, true));
    expect(await screen.findByText("trips")).toBeInTheDocument();
  });
});

describe("as a drop target", () => {
  it("moves items dropped on it once it is showing a real folder", async () => {
    const folders = [node({ id: 1, title: "trips" })];
    mocked.moveItems.mockResolvedValue({ moved: 1, batchId: "b1", errors: [] });
    renderGridMode(folders, <DragStarter payload={{ kind: "items", itemIds: [7] }} />);

    await userEvent.click(await screen.findByText("Everything"));
    await userEvent.click(await screen.findByRole("button", { name: /trips/ }));
    await screen.findByText("trips");

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    const dropZone = document.querySelector(".grid-scroll")!.parentElement!;
    fireEvent.dragEnter(dropZone);
    fireEvent.dragOver(dropZone);
    fireEvent.drop(dropZone);

    await waitFor(() => expect(mocked.moveItems).toHaveBeenCalledWith([7], 1));
  });

  it("refuses a drop while showing Everything — there is nowhere real to file into", async () => {
    renderGridMode([], <DragStarter payload={{ kind: "items", itemIds: [7] }} />);
    await screen.findByText("Everything");

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    const dropZone = document.querySelector(".grid-scroll")!.parentElement!;
    fireEvent.dragEnter(dropZone);
    fireEvent.dragOver(dropZone);
    fireEvent.drop(dropZone);

    expect(mocked.moveItems).not.toHaveBeenCalled();
  });
});
