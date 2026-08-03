/**
 * The folder band. Two things it must get right, from docs/DESIGN.md §2:
 * expanded state is global rather than per folder, and it looks right with no
 * archetype at all — which is the default and commonest state, since the app
 * ships with none.
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
  const folder = folderNode({ status: "wip" });
  const result = renderWithProviders(
    <FolderBand
      folder={folder}
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
      {...over}
    />,
    { library: fakeLibrary() },
  );
  return { ...result, onExpandedChange };
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

  it("expands through the caller, so the state can be global", async () => {
    const { onExpandedChange } = renderBand();
    await userEvent.click(screen.getByRole("button", { name: /Expand folder details/ }));
    expect(onExpandedChange).toHaveBeenCalledWith(true);
  });
});

describe("expanded, with no archetype", () => {
  it("shows the cover, the counts and an add-field control — not blank labels", async () => {
    renderBand({ expanded: true });

    expect(await screen.findByText(/add field/)).toBeInTheDocument();
    expect(screen.getByLabelText("Folder notes")).toBeInTheDocument();
    expect(screen.getByText(/4 items here/)).toBeInTheDocument();
    expect(screen.getByText(/2 subfolders/)).toBeInTheDocument();
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

  it("edits an archetype field in place", async () => {
    mocked.getFolder.mockResolvedValue(
      detail({
        archetypeId: 3,
        archetypeName: "Trip",
        fields: [{ key: "city", ordinal: 0, value: "" }],
      }),
    );
    renderBand({ expanded: true });

    await userEvent.click(await screen.findByRole("button", { name: "—" }));
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
});

describe("status", () => {
  it("sets the folder's status through the command", async () => {
    renderBand();

    await userEvent.click(await screen.findByRole("button", { name: "Folder status" }));
    await userEvent.click(await screen.findByRole("menuitem", { name: "Active" }));

    await waitFor(() => expect(mocked.setFolderStatus).toHaveBeenCalledWith(1, "active"));
  });
});
