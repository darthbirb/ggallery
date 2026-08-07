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
    expect(await screen.findByRole("button", { name: "Folder Status" })).toHaveTextContent(
      "WIP",
    );
    expect(screen.getByText(/4 here/)).toBeInTheDocument();
    // Nothing from the expanded half is on screen.
    expect(screen.queryByLabelText("Folder Notes")).toBeNull();
  });

  it("shows no status chip when the folder is Active — absence means nothing to say", async () => {
    mocked.getFolder.mockResolvedValue(detail({ status: "active" }));
    renderBand({ folder: folderNode({ status: "active" }) });
    await screen.findByText(/4 here/);
    expect(screen.queryByRole("button", { name: "Folder Status" })).toBeNull();
  });

  it("expands through the caller, so the state can be global", async () => {
    const { onExpandedChange } = renderBand();
    await userEvent.click(screen.getByRole("button", { name: /Expand Folder Details/ }));
    expect(onExpandedChange).toHaveBeenCalledWith(true);
  });

  it("carries the tile-size and here-only/all-items toggle, moved from the window bar", async () => {
    const { onRecursiveChange } = renderBand();
    expect(screen.getByLabelText("Tile Size")).toBeInTheDocument();

    // `recursive` starts true — "All Items" is the current state, and
    // clicking it scopes down to here only.
    const toggle = screen.getByRole("button", { name: "All Items" });
    expect(toggle).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(toggle);
    expect(onRecursiveChange).toHaveBeenCalledWith(false);
  });

  it("reads 'Here only' and stays pressed once scoped to just this folder", async () => {
    const { onRecursiveChange } = renderBand({ recursive: false });

    const toggle = screen.getByRole("button", { name: "Here Only" });
    expect(toggle).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(toggle);
    expect(onRecursiveChange).toHaveBeenCalledWith(true);
  });

  it("renders a plain label with no chevron or grid controls for a non-folder scope", () => {
    renderBand({ folder: null, scopeLabel: "Everything", itemCount: 42 });
    expect(screen.getByText("Everything")).toBeInTheDocument();
    expect(screen.getByText(/42 items/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Expand Folder Details/ })).toBeNull();
    expect(screen.queryByRole("button", { name: /All Items|Here Only/ })).toBeNull();
    // Tile size still applies to every scope.
    expect(screen.getByLabelText("Tile Size")).toBeInTheDocument();
  });
});

describe("expanded, with no archetype", () => {
  it("shows the cover, the counts once and an add-label control — not blank labels", async () => {
    renderBand({ expanded: true });

    expect(await screen.findByText(/Add Label/)).toBeInTheDocument();
    expect(screen.getByLabelText("Folder Notes")).toBeInTheDocument();
    // The header's "4 here · 12 below · 2 subfolders" is the only counts
    // line — nothing in the expanded panel repeats it.
    expect(screen.getAllByText(/4 here/)).toHaveLength(1);
    // No archetype exists, so no archetype control is offered at all.
    expect(screen.queryByText(/Apply an archetype/)).toBeNull();
  });

  it("adds a one-off label with both its name and value in one motion", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/Add Label/));
    // Enter in the name field moves to the value field rather than
    // committing — a label with no value typed yet is not necessarily done.
    await userEvent.type(screen.getByLabelText("New Label Name"), "city{Enter}");
    await userEvent.type(screen.getByLabelText("New Label Value"), "Lisbon{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", "Lisbon"),
    );
  });

  it("still allows a label with an empty value — it exists and renders anyway", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/Add Label/));
    await userEvent.type(screen.getByLabelText("New Label Name"), "city{Enter}{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", ""),
    );
  });

  it("shows a parent folder's labels and flags, greyed and ahead of this folder's own", async () => {
    mocked.folderInheritedTags.mockResolvedValue([
      { tagId: 9, key: null, value: "Family Trip", originId: 100, originIsTitle: false },
      { tagId: 10, key: "country", value: "Portugal", originId: 100, originIsTitle: false },
    ]);
    renderBand({ expanded: true });

    expect(await screen.findByText("Family Trip")).toBeInTheDocument();
    expect(await screen.findByText("Portugal")).toBeInTheDocument();
    // Inherited, not this folder's own — there is nothing to remove or
    // rename here, so no aria-label offers to.
    expect(screen.queryByRole("button", { name: "Remove Family Trip" })).toBeNull();
    expect(screen.queryByRole("button", { name: /country/ })).toBeNull();
  });

  it("never shows an ancestor's own folder-name tag as an inherited chip", async () => {
    mocked.folderInheritedTags.mockResolvedValue([
      // Folder 100's own title tag, inherited — must not render.
      { tagId: 9, key: null, value: "People", originId: 100, originIsTitle: true },
      // A different ancestor's manual flag that happens to say the same
      // word — a deliberate choice, not the folder leaking through, so it
      // stays.
      { tagId: 9, key: null, value: "People", originId: 50, originIsTitle: false },
    ]);
    renderBand({ expanded: true });

    await screen.findByText(/Add Label/);
    // Only the manual contribution renders — the title contribution is
    // suppressed structurally, not by comparing display text.
    expect(screen.getAllByText("People")).toHaveLength(1);
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

    await userEvent.click(await screen.findByText(/Add Tag/));
    await userEvent.type(screen.getByLabelText("New Tag"), "summer{Enter}");

    await waitFor(() => expect(mocked.addFolderFlag).toHaveBeenCalledWith(1, "summer"));
  });

  it("keeps the note on screen after it is saved — a round trip, not just the write", async () => {
    // Asserting only that `setFolderNotes` was called (the old shape of
    // this test) passes even if the note is written but never re-read —
    // exactly the bug this guards against.
    const { rerenderWithProviders, library } = renderBand({ expanded: true });

    await userEvent.click(await screen.findByLabelText("Folder Notes"));
    await userEvent.type(screen.getByLabelText("Folder Notes"), "a real note");
    await userEvent.tab();

    await waitFor(() =>
      expect(mocked.setFolderNotes).toHaveBeenCalledWith(1, "a real note"),
    );
    // The commit must ask the app to refetch, the same as every neighbouring
    // folder operation already does.
    expect(library.refreshFolders).toHaveBeenCalled();

    // What that refetch produces in the real app: `refreshToken` bumps, and
    // the next `getFolder` reflects the saved value.
    mocked.getFolder.mockResolvedValueOnce(detail({ notes: "a real note" }));
    const folder = folderNode({ status: "wip" });
    rerenderWithProviders(
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
        expanded
        onExpandedChange={vi.fn()}
        thumbsDir="D:/thumbs"
        refreshToken={1}
        onOpen={vi.fn()}
        tileHeight={132}
        onTileHeightChange={vi.fn()}
        recursive
        onRecursiveChange={vi.fn()}
      />,
    );

    expect(await screen.findByText("a real note")).toBeInTheDocument();
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

  it("shows its own crumb for a top-level folder — a folder always shows where it sits", async () => {
    const folder = folderNode({ parentId: null });
    renderBand({ folder, folders: [folder], expanded: true });
    await screen.findByText(/Add Label/);
    // Once in the header, once as the breadcrumb's single (and only) crumb.
    expect(screen.getAllByText("Trips")).toHaveLength(2);
  });
});

describe("status", () => {
  it("sets the folder's status through the command", async () => {
    renderBand();

    await userEvent.click(await screen.findByRole("button", { name: "Folder Status" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Active" }));

    await waitFor(() => expect(mocked.setFolderStatus).toHaveBeenCalledWith(1, "active"));
  });
});
