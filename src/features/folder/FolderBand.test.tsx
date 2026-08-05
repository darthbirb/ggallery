/**
 * The folder band. Things it must get right, from docs/DESIGN.md §2:
 * expanded state is global rather than per folder, it looks right with no
 * archetype at all — the default and commonest state, since the app ships
 * with none — and counts appear exactly once.
 */

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, folderNode, renderWithProviders, topLevelNode } from "../../test/harness";
import type { FolderDetail } from "../../lib/types";
import { FolderBand } from "./FolderBand";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

function detail(over: Partial<FolderDetail> = {}): FolderDetail {
  return {
    id: 1,
    title: "Trips",
    parentId: 100,
    status: "wip",
    favorite: false,
    notes: null,
    lastAddedAt: null,
    directCount: 4,
    totalCount: 12,
    subfolderCount: 2,
    archetypeId: null,
    archetypeName: null,
    fields: [],
    flags: [],
    coverThumb: null,
    coverItemId: null,
    ...over,
  };
}

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.getFolder.mockResolvedValue(detail());
  mocked.folderInheritedTags.mockResolvedValue([]);
  mocked.assetUrl.mockReturnValue("asset://thumb");
  mocked.setFolderStatus.mockResolvedValue(undefined);
  mocked.setFolderLabel.mockResolvedValue(undefined);
  mocked.addFolderFlag.mockResolvedValue(undefined);
  mocked.setFolderNotes.mockResolvedValue(undefined);
  mocked.setFolderFavorite.mockResolvedValue(undefined);
});

function renderBand(over: Partial<React.ComponentProps<typeof FolderBand>> = {}) {
  const onExpandedChange = vi.fn();
  const onRecursiveChange = vi.fn();
  const onTileHeightChange = vi.fn();
  const folder = folderNode({ status: "wip" });
  const result = renderWithProviders(
    <FolderBand
      folder={folder}
      folders={[topLevelNode(), folder]}
      scopeLabel="Everything"
      itemCount={0}
      statuses={[
        { key: "active", label: "Active", colour: "#6b7280", ordinal: 0 },
        { key: "wip", label: "WIP", colour: "#eab308", ordinal: 1 },
      ]}
      archetypes={[]}
      expanded={false}
      onExpandedChange={onExpandedChange}
      thumbsDir="D:/thumbs"
      refreshToken={0}
      onOpen={vi.fn()}
      tileHeight={132}
      onTileHeightChange={onTileHeightChange}
      recursive
      onRecursiveChange={onRecursiveChange}
      {...over}
    />,
    { library: fakeLibrary() },
  );
  return { ...result, onExpandedChange, onRecursiveChange, onTileHeightChange };
}

describe("collapsed", () => {
  it("is one line: title, status chip and counts", async () => {
    renderBand();
    expect(screen.getByText("Trips")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Folder status" })).toHaveTextContent(
      "WIP",
    );
    expect(screen.getByText(/4 here/)).toBeInTheDocument();
    // Nothing from the expanded half is on screen.
    expect(screen.queryByLabelText("Folder notes")).toBeNull();
  });

  it("shows no status chip when the folder is Active — absence means nothing to say", async () => {
    mocked.getFolder.mockResolvedValue(detail({ status: "active" }));
    renderBand({ folder: folderNode({ status: "active" }) });
    await screen.findByText(/4 here/);
    expect(screen.queryByRole("button", { name: "Folder status" })).toBeNull();
  });

  it("expands through the caller, so the state can be global", async () => {
    const { onExpandedChange } = renderBand();
    await userEvent.click(screen.getByRole("button", { name: /Expand folder details/ }));
    expect(onExpandedChange).toHaveBeenCalledWith(true);
  });

  it("carries the tile-size and here-only/all-items toggle, moved from the window bar", async () => {
    const { onRecursiveChange } = renderBand();
    expect(screen.getByLabelText("Tile size")).toBeInTheDocument();

    // `recursive` starts true — "All items" is the current state, and
    // clicking it scopes down to here only.
    const toggle = screen.getByRole("button", { name: "All items" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(toggle);
    expect(onRecursiveChange).toHaveBeenCalledWith(false);
  });

  it("reads 'Here only' and stays pressed once scoped to just this folder", async () => {
    const { onRecursiveChange } = renderBand({ recursive: false });

    const toggle = screen.getByRole("button", { name: "Here only" });
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(toggle);
    expect(onRecursiveChange).toHaveBeenCalledWith(true);
  });

  it("renders a plain label with no chevron or grid controls for a non-folder scope", () => {
    renderBand({ folder: null, scopeLabel: "Everything", itemCount: 42 });
    expect(screen.getByText("Everything")).toBeInTheDocument();
    expect(screen.getByText(/42 items/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Expand folder details/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /All items|Here only/ })).toBeNull();
    // Tile size still applies to every scope.
    expect(screen.getByLabelText("Tile size")).toBeInTheDocument();
  });
});

describe("expanded, with no archetype", () => {
  it("shows the cover, the counts once and an add-label control — not blank labels", async () => {
    renderBand({ expanded: true });

    expect(await screen.findByText(/add label/)).toBeInTheDocument();
    expect(screen.getByLabelText("Folder notes")).toBeInTheDocument();
    // The header's "4 here · 12 below · 2 subfolders" is the only counts
    // line — nothing in the expanded panel repeats it.
    expect(screen.getAllByText(/4 here/)).toHaveLength(1);
    // No archetype exists, so no archetype control is offered at all.
    expect(screen.queryByText(/Apply an archetype/)).toBeNull();
  });

  it("adds a one-off label with both its name and value in one motion", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/add label/));
    // Enter in the name field moves to the value field rather than
    // committing — a label with no value typed yet is not necessarily done.
    await userEvent.type(screen.getByLabelText("New label name"), "city{Enter}");
    await userEvent.type(screen.getByLabelText("New label value"), "Lisbon{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", "Lisbon"),
    );
  });

  it("still allows a label with an empty value — it exists and renders anyway", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/add label/));
    await userEvent.type(screen.getByLabelText("New label name"), "city{Enter}{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", ""),
    );
  });

  it("shows a parent folder's labels and flags, greyed and ahead of this folder's own", async () => {
    mocked.folderInheritedTags.mockResolvedValue([
      { tagId: 9, key: null, value: "People", originId: 100 },
      { tagId: 10, key: "country", value: "Portugal", originId: 100 },
    ]);
    renderBand({ expanded: true });

    expect(await screen.findByText("People")).toBeInTheDocument();
    expect(await screen.findByText("Portugal")).toBeInTheDocument();
    // Inherited, not this folder's own — there is nothing to remove or
    // rename here, so no aria-label offers to.
    expect(screen.queryByRole("button", { name: "Remove People" })).toBeNull();
    expect(screen.queryByRole("button", { name: /country/ })).toBeNull();
  });

  it("edits an archetype field in place, as a chip beside the tags", async () => {
    mocked.getFolder.mockResolvedValue(
      detail({
        archetypeId: 3,
        archetypeName: "Trip",
        fields: [{ key: "city", ordinal: 0, value: "" }],
      }),
    );
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByRole("button", { name: /city/ }));
    await userEvent.type(screen.getByLabelText("city"), "lisbon{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", "lisbon"),
    );
  });

  it("adds a flag to the folder's tag set", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/add tag/));
    await userEvent.type(screen.getByLabelText("New tag"), "summer{Enter}");

    await waitFor(() => expect(mocked.addFolderFlag).toHaveBeenCalledWith(1, "summer"));
  });

  it("commits a note typed into the growing line", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByLabelText("Folder notes"));
    await userEvent.type(screen.getByLabelText("Folder notes"), "a real note");
    await userEvent.tab();

    await waitFor(() =>
      expect(mocked.setFolderNotes).toHaveBeenCalledWith(1, "a real note"),
    );
  });
});

describe("ancestry", () => {
  it("shows the same breadcrumb the item details panel uses, folder itself last", async () => {
    const people = folderNode({ id: 2, title: "People", parentId: null });
    const trip = folderNode({ id: 1, title: "Trips", parentId: 2 });
    renderBand({ folder: trip, folders: [people, trip], expanded: true });

    expect(await screen.findByText("People")).toBeInTheDocument();
    // "Trips" is both the header title and the breadcrumb's last crumb.
    expect(screen.getAllByText("Trips")).toHaveLength(2);
  });

  it("shows no breadcrumb for a top-level folder — nothing sits above itself", async () => {
    const folder = folderNode({ parentId: null });
    renderBand({ folder, folders: [folder], expanded: true });
    await screen.findByText(/add label/);
    expect(screen.getAllByText("Trips")).toHaveLength(1);
  });
});

describe("status", () => {
  it("sets the folder's status through the command", async () => {
    renderBand();

    await userEvent.click(await screen.findByRole("button", { name: "Folder status" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Active" }));

    await waitFor(() => expect(mocked.setFolderStatus).toHaveBeenCalledWith(1, "active"));
  });
});
