/**
 * The folder band. Things it must get right, from docs/DESIGN.md §2:
 * expanded state is global rather than per folder, it looks right with no
 * archetype at all — the default and commonest state, since the app ships
 * with none — and counts appear exactly once.
 */

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, folderNode, renderWithProviders } from "../../test/harness";
import type { FolderDetail } from "../../lib/types";
import { FolderBand } from "./FolderBand";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

function detail(over: Partial<FolderDetail> = {}): FolderDetail {
  return {
    id: 1,
    relPath: "trips",
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

  it("carries the tile-size and this-folder-only controls, moved from the window bar", async () => {
    const { onRecursiveChange } = renderBand();
    expect(screen.getByLabelText("Tile size")).toBeInTheDocument();

    const checkbox = screen.getByRole("checkbox", { name: /this folder only/ });
    await userEvent.click(checkbox);
    expect(onRecursiveChange).toHaveBeenCalledWith(false);
  });

  it("renders a plain label with no chevron or grid controls for a non-folder scope", () => {
    renderBand({ folder: null, scopeLabel: "Everything", itemCount: 42 });
    expect(screen.getByText("Everything")).toBeInTheDocument();
    expect(screen.getByText(/42 items/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /Expand folder details/ })).toBeNull();
    expect(screen.queryByRole("checkbox", { name: /this folder only/ })).toBeNull();
    // Tile size still applies to every scope.
    expect(screen.getByLabelText("Tile size")).toBeInTheDocument();
  });
});

describe("expanded, with no archetype", () => {
  it("shows the cover, the counts once and an add-field control — not blank labels", async () => {
    renderBand({ expanded: true });

    expect(await screen.findByText(/add field/)).toBeInTheDocument();
    expect(screen.getByLabelText("Folder notes")).toBeInTheDocument();
    // The header's "4 here · 12 below · 2 subfolders" is the only counts
    // line — nothing in the expanded panel repeats it.
    expect(screen.getAllByText(/4 here/)).toHaveLength(1);
    // No archetype exists, so no archetype control is offered at all.
    expect(screen.queryByText(/Apply an archetype/)).toBeNull();
  });

  it("adds a one-off field as a label with an empty value", async () => {
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByText(/add field/));
    await userEvent.type(screen.getByLabelText("New field name"), "city{Enter}");

    await waitFor(() =>
      expect(mocked.setFolderLabel).toHaveBeenCalledWith(1, "city", ""),
    );
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

    await userEvent.type(
      await screen.findByLabelText("Add a tag to this folder"),
      "summer{Enter}",
    );

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

describe("status", () => {
  it("sets the folder's status through the command", async () => {
    renderBand();

    await userEvent.click(await screen.findByRole("button", { name: "Folder status" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Active" }));

    await waitFor(() => expect(mocked.setFolderStatus).toHaveBeenCalledWith(1, "active"));
  });
});
