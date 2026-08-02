/**
 * The navigation panel's two hard requirements, from docs/DESIGN.md §2:
 * the library root is not a node in the tree, and the tree never reorders.
 */

import { screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import {
  EVERYTHING_SCOPE,
  fakeLibrary,
  folderNode,
  renderWithProviders,
  rootNode,
} from "../../test/harness";
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
    rootNode(),
    folderNode({ id: 1, relPath: "alps", title: "Alps" }),
    folderNode({ id: 2, relPath: "borneo", title: "Borneo" }),
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
      {...over}
    />,
    { library: fakeLibrary({ folders }) },
  );
  return { ...result, onScope };
}

describe("navigation roots", () => {
  it("puts Everything, Loose items and Favourites above the tree", () => {
    renderNav();
    const nav = screen.getByRole("navigation");
    expect(within(nav).getByRole("button", { name: /Everything/ })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: /Loose items/ })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: /Favourites/ })).toBeInTheDocument();
  });

  it("never shows the library root as a folder row", () => {
    renderNav();
    // The root's title exists in the data; it must not be a row.
    expect(screen.queryByRole("button", { name: /^Library/ })).toBeNull();
  });

  it("asks for the three roots as three different queries", async () => {
    const { onScope } = renderNav();

    await userEvent.click(screen.getByRole("button", { name: /Loose items/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "loose",
      folder: null,
      recursive: false,
    });

    await userEvent.click(screen.getByRole("button", { name: /Favourites/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "favourites",
      folder: null,
      recursive: true,
    });
  });

  it("renders an empty tree as empty, not as a root node", () => {
    renderNav({ folders: [rootNode()] });
    expect(screen.getByText(/No folders yet/)).toBeInTheDocument();
  });
});

describe("the tree", () => {
  it("does not reorder when a folder is pinned", () => {
    const folders = [
      rootNode(),
      folderNode({ id: 1, relPath: "alps", title: "Alps" }),
      folderNode({ id: 2, relPath: "borneo", title: "Borneo", favorite: true }),
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
        rootNode(),
        folderNode({ id: 1, relPath: "alps", title: "Alps", status: "wip" }),
        folderNode({ id: 2, relPath: "borneo", title: "Borneo", status: "done" }),
      ],
    });

    expect(screen.getAllByTitle("Work in progress")).toHaveLength(1);
  });

  it("opens a folder as a folder-scoped view", async () => {
    const { onScope } = renderNav();
    await userEvent.click(screen.getByRole("button", { name: /Alps/ }));
    expect(onScope).toHaveBeenCalledWith({
      kind: "folder",
      folder: "alps",
      recursive: true,
    });
  });
});

describe("folding", () => {
  it("is folded by a visible control, never by a keypress alone", async () => {
    const onFoldedChange = vi.fn();
    renderNav({ onFoldedChange });

    await userEvent.click(
      screen.getByRole("button", { name: "Hide the navigation panel" }),
    );
    expect(onFoldedChange).toHaveBeenCalledWith(true);
  });

  it("keeps every root reachable from the folded strip", () => {
    renderNav({ folded: true });
    expect(screen.getByRole("button", { name: "Everything" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Loose items" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Favourites" })).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Show the navigation panel" }),
    ).toBeInTheDocument();
  });
});
