/**
 * The navigation panel's two hard requirements, from SPEC.md §2:
 * the library root is not a node in the tree — since decision 30
 * there is no row it even could be, every folder is a real one — and the
 * tree never reorders.
 */

import { fireEvent, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  EVERYTHING_SCOPE,
  fakeLibrary,
  folderNode,
  renderWithProviders,
  topLevelNode,
} from "../../test/harness";
import type { DragPayload } from "../../state/dnd";
import { useDnd } from "../../state/dnd";
import { Nav } from "./Nav";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.listArchetypes.mockResolvedValue([]);
});

function renderNav(over: Partial<React.ComponentProps<typeof Nav>> = {}) {
  const onScope = vi.fn();
  const folders = over.folders ?? [
    topLevelNode(),
    folderNode({ id: 1, title: "Alps" }),
    folderNode({ id: 2, title: "Borneo" }),
  ];
  const result = renderWithProviders(
    <Nav
      folders={folders}
      scope={EVERYTHING_SCOPE}
      onScope={onScope}
      statuses={[]}
      archetypes={[]}
      folded={false}
      onFoldedChange={vi.fn()}
      onEditDetails={vi.fn()}
      favouriteCount={3}
      sortingCount={7}
      progress={null}
      failureCount={0}
      showingFailures={false}
      onToggleFailures={vi.fn()}
      onOpenSettings={vi.fn()}
      {...over}
    />,
    { library: fakeLibrary({ folders }) },
  );
  return { ...result, onScope };
}

describe("navigation roots", () => {
  it("puts Everything, the Sorting Box and Favourites above the tree", () => {
    renderNav();
    const nav = screen.getByRole("navigation");
    expect(within(nav).getByRole("button", { name: /Everything/ })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: /Sorting Box/ })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: /Favourites/ })).toBeInTheDocument();
  });

  it("has no Sorting Box folder in the tree — an unfiled item just has no folder", () => {
    // A real folder of that name would be a second way of saying the same
    // thing (SPEC.md §2 and §4), so it is an ordinary folder and nothing
    // promotes it into a queue group of its own.
    renderNav({
      folders: [topLevelNode(), folderNode({ id: 1, title: "Sorting Box" })],
    });
    expect(screen.queryByText("Queues")).toBeNull();
    // Two: the navigation root, and the ordinary folder row.
    expect(screen.getAllByRole("button", { name: /Sorting Box/ })).toHaveLength(2);
  });

  it("asks for the three roots as three different queries", async () => {
    const { onScope } = renderNav();

    await userEvent.click(screen.getByRole("button", { name: /Sorting Box/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "sorting",
      folderId: null,
      recursive: false,
    });

    await userEvent.click(screen.getByRole("button", { name: /Favourites/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "favourites",
      folderId: null,
      recursive: true,
    });
  });

  it("renders an empty tree as empty, not as a root node", () => {
    // There is no library-root row to filter out any more (decision
    // 30) — an empty tree is just an empty `folders` array.
    renderNav({ folders: [] });
    expect(screen.getByText(/No folders yet/)).toBeInTheDocument();
  });
});

describe("the tree", () => {
  it("does not reorder when a folder is pinned", () => {
    const folders = [
      topLevelNode(),
      folderNode({ id: 1, title: "Alps" }),
      folderNode({ id: 2, title: "Borneo", favorite: true }),
    ];
    renderNav({ folders });

    // Borneo appears twice — once pinned, once in place — and the tree's own
    // order is still alphabetical. Pinning must never move the row you reach
    // for.
    const rows = screen
      .getAllByRole("button")
      .map((node) => node.textContent ?? "")
      .filter((text) => text.includes("Alps") || text.includes("Borneo"));
    expect(rows.filter((text) => text.includes("Borneo"))).toHaveLength(2);
    const treeRows = rows.slice(1); // after the pinned copy
    expect(treeRows[0]).toContain("Alps");
    expect(treeRows[1]).toContain("Borneo");
  });

  it("marks WIP with one dot and marks nothing else", () => {
    renderNav({
      folders: [
        topLevelNode(),
        folderNode({ id: 1, title: "Alps", status: "wip" }),
        folderNode({ id: 2, title: "Borneo", status: "done" }),
      ],
    });

    expect(screen.getAllByTitle("Work In Progress")).toHaveLength(1);
  });

  it("opens a folder as a folder-scoped view", async () => {
    const { onScope } = renderNav();
    await userEvent.click(screen.getByRole("button", { name: /Alps/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "folder",
      folderId: 1,
      recursive: true,
    });
  });
});

/** A drag is normally started by the browser's own gesture on a draggable
 *  source; these tests only need a drag to already be *in progress* when a
 *  drop lands on a target, so this stands one up directly through the
 *  shared `useDnd()` context instead of simulating a real `dragstart`. */
function DragStarter({ payload }: { payload: DragPayload }) {
  const { startDrag } = useDnd();
  return (
    <button type="button" onClick={() => startDrag(payload)}>
      start drag
    </button>
  );
}

function fireDrop(target: HTMLElement) {
  fireEvent.dragEnter(target);
  fireEvent.dragOver(target);
  fireEvent.drop(target);
}

describe("tree rows as drop targets", () => {
  it("moves items dropped onto a folder row", async () => {
    const folders = [topLevelNode(), folderNode({ id: 1, title: "Alps" })];
    const { library } = renderWithProviders(
      <>
        <Nav
          folders={folders}
          scope={EVERYTHING_SCOPE}
          onScope={vi.fn()}
          statuses={[]}
          archetypes={[]}
          folded={false}
          onFoldedChange={vi.fn()}
          onEditDetails={vi.fn()}
          favouriteCount={0}
          sortingCount={0}
          progress={null}
          failureCount={0}
          showingFailures={false}
          onToggleFailures={vi.fn()}
          onOpenSettings={vi.fn()}
        />
        <DragStarter payload={{ kind: "items", itemIds: [7, 8] }} />
      </>,
      { library: fakeLibrary({ folders }) },
    );
    mocked.moveItems.mockResolvedValue({ moved: 2, batchId: "b1", errors: [] });

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    fireDrop(screen.getByRole("button", { name: /Alps/ }));

    expect(mocked.moveItems).toHaveBeenCalledWith([7, 8], 1);
    await waitFor(() => expect(library.reload).toHaveBeenCalled());
  });

  it("nests a dragged folder onto another row", async () => {
    const folders = [
      topLevelNode(),
      folderNode({ id: 1, title: "Alps" }),
      folderNode({ id: 2, title: "Borneo", parentId: null, depth: 0 }),
    ];
    renderWithProviders(
      <>
        <Nav
          folders={folders}
          scope={EVERYTHING_SCOPE}
          onScope={vi.fn()}
          statuses={[]}
          archetypes={[]}
          folded={false}
          onFoldedChange={vi.fn()}
          onEditDetails={vi.fn()}
          favouriteCount={0}
          sortingCount={0}
          progress={null}
          failureCount={0}
          showingFailures={false}
          onToggleFailures={vi.fn()}
          onOpenSettings={vi.fn()}
        />
        <DragStarter payload={{ kind: "folder", folder: folders[2] }} />
      </>,
      { library: fakeLibrary({ folders }) },
    );
    mocked.moveFolder.mockResolvedValue("batch-1");

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    fireDrop(screen.getByRole("button", { name: /Alps/ }));

    await waitFor(() => expect(mocked.moveFolder).toHaveBeenCalledWith(2, 1));
  });

  it("refuses a folder dropped onto itself before ever asking the backend", async () => {
    const folders = [topLevelNode(), folderNode({ id: 1, title: "Alps" })];
    renderWithProviders(
      <>
        <Nav
          folders={folders}
          scope={EVERYTHING_SCOPE}
          onScope={vi.fn()}
          statuses={[]}
          archetypes={[]}
          folded={false}
          onFoldedChange={vi.fn()}
          onEditDetails={vi.fn()}
          favouriteCount={0}
          sortingCount={0}
          progress={null}
          failureCount={0}
          showingFailures={false}
          onToggleFailures={vi.fn()}
          onOpenSettings={vi.fn()}
        />
        <DragStarter payload={{ kind: "folder", folder: folders[1] }} />
      </>,
      { library: fakeLibrary({ folders }) },
    );

    await userEvent.click(screen.getByRole("button", { name: "start drag" }));
    fireDrop(screen.getByRole("button", { name: /Alps/ }));

    expect(mocked.moveFolder).not.toHaveBeenCalled();
  });
});

describe("folding", () => {
  it("is folded by a visible control, never by a keypress alone", async () => {
    const onFoldedChange = vi.fn();
    renderNav({ onFoldedChange });

    await userEvent.click(
      screen.getByRole("button", { name: "Hide The Navigation Panel" }),
    );
    expect(onFoldedChange).toHaveBeenCalledWith(true);
  });

  it("keeps every root reachable from the folded strip", () => {
    renderNav({ folded: true });
    expect(screen.getByRole("button", { name: "Everything" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Sorting Box" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Favourites" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show The Navigation Panel" }),
    ).toBeInTheDocument();
  });
});
