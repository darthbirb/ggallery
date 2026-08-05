/**
 * The pane's Folders mode — DESIGN.md §2 *Folders mode*: single click drills
 * in without moving the main grid, double click does move it, a "+ New
 * folder" tile is always present and creates inline, the filter box
 * searches flat and offers to create what does not match, and tiles accept
 * drops the same way every other target in the app does.
 */

import { fireEvent, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, folderDetail, renderWithProviders } from "../../test/harness";
import type { DragPayload } from "../../state/dnd";
import { useDnd } from "../../state/dnd";
import type { FolderNode } from "../../lib/types";
import { FoldersMode } from "./FoldersMode";

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
  mocked.getFolder.mockResolvedValue(folderDetail());
  mocked.createFolder.mockResolvedValue(99);
});

function DragStarter({ payload }: { payload: DragPayload }) {
  const { startDrag } = useDnd();
  return (
    <button type="button" onClick={() => startDrag(payload)}>
      start drag
    </button>
  );
}

function renderFoldersMode(
  folders: FolderNode[],
  over: Partial<React.ComponentProps<typeof FoldersMode>> = {},
  extra: React.ReactNode = null,
) {
  const onOpenInMain = vi.fn();
  const result = renderWithProviders(
    <>
      <FoldersMode
        mode="folders"
        onModeChange={vi.fn()}
        onClose={vi.fn()}
        maximised={false}
        onMaximisedChange={vi.fn()}
        folders={folders}
        refreshToken={0}
        thumbsDir="D:/thumbs"
        onOpenInMain={onOpenInMain}
        {...over}
      />
      {extra}
    </>,
    { library: fakeLibrary({ folders }) },
  );
  return { ...result, onOpenInMain };
}

describe("drilling versus opening", () => {
  it("single click drills in without moving the main grid", async () => {
    const folders = [node({ id: 1, title: "trips" }), node({ id: 2, title: "alps", parentId: 1, depth: 1 })];
    const { onOpenInMain } = renderFoldersMode(folders);

    await userEvent.click(await screen.findByText("trips"));

    expect(onOpenInMain).not.toHaveBeenCalled();
    expect(await screen.findByText("alps")).toBeInTheDocument();
  });

  it("double click navigates the main grid there", async () => {
    const folders = [node({ id: 1, title: "trips" })];
    const { onOpenInMain } = renderFoldersMode(folders);

    fireEvent.doubleClick(await screen.findByText("trips"));

    expect(onOpenInMain).toHaveBeenCalledWith(folders[0]);
  });
});

describe("inline folder creation", () => {
  it("always offers a New folder tile, and creates on Enter", async () => {
    const folders = [node({ id: 1, title: "trips" })];
    renderFoldersMode(folders);

    await userEvent.click(await screen.findByText(/New folder in the top level/));
    const input = screen.getByPlaceholderText("Folder name");
    await userEvent.type(input, "beach{Enter}");

    await waitFor(() =>
      expect(mocked.createFolder).toHaveBeenCalledWith(null, "beach", null),
    );
  });

  it("creates only once when the Create button is clicked, not twice via blur", async () => {
    // Clicking the button both blurs the input (moving focus away) and
    // fires the button's own click — both call `submit`, and only the first
    // may act, or the folder gets created twice.
    const folders = [node({ id: 1, title: "trips" })];
    renderFoldersMode(folders);

    await userEvent.click(await screen.findByText(/New folder in the top level/));
    await userEvent.type(screen.getByPlaceholderText("Folder name"), "beach");
    await userEvent.click(screen.getByRole("button", { name: "Create" }));

    await waitFor(() => expect(mocked.createFolder).toHaveBeenCalledTimes(1));
  });

  it("offers to create what the filter found nothing matching", async () => {
    const folders = [node({ id: 1, title: "trips" })];
    renderFoldersMode(folders);

    await userEvent.type(screen.getByPlaceholderText("Filter by title or path…"), "roo");
    const createRow = await screen.findByText(/Create/);
    expect(createRow.textContent).toContain("roo");
    expect(createRow.textContent).toContain("the top level");

    await userEvent.click(createRow);
    await waitFor(() =>
      expect(mocked.createFolder).toHaveBeenCalledWith(null, "roo", null),
    );
  });
});

describe("tiles as drop targets", () => {
  it("moves items dropped onto a tile", async () => {
    const folders = [node({ id: 1, title: "trips", totalCount: 3 })];
    mocked.moveItems.mockResolvedValue({ moved: 1, batchId: "b1", errors: [] });
    renderFoldersMode(folders, {}, <DragStarter payload={{ kind: "items", itemIds: [7] }} />);

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    const tile = await screen.findByText("trips");
    fireEvent.dragEnter(tile);
    fireEvent.dragOver(tile);
    fireEvent.drop(tile);

    await waitFor(() => expect(mocked.moveItems).toHaveBeenCalledWith([7], 1));
  });
});
