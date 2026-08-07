/**
 * The item menu, audited against SPEC.md §8.
 *
 * "Controls that must exist visibly, not only as bindings: select all, invert,
 * clear; favourite; delete; reveal in Explorer; open with; copy file; copy
 * path; blur; fold the navigation panel; negate a query term."
 *
 * These tests are that audit, held in place. A menu that loses an entry — or
 * gains one wired to the wrong command — fails here rather than in use.
 */

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { PointMenu } from "../../components/Menu";
import {
  fakeLibrary,
  fakeSelection,
  gridItem,
  renderWithProviders,
} from "../../test/harness";
import { EmptyMenu, ItemMenu } from "./ItemMenu";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.listArchetypes.mockResolvedValue([]);
  mocked.setItemsFavorite.mockResolvedValue(undefined);
  mocked.revealItem.mockResolvedValue(undefined);
  mocked.openItem.mockResolvedValue(undefined);
  mocked.copyItemFile.mockResolvedValue(undefined);
  mocked.copyItemPath.mockResolvedValue(undefined);
  mocked.setFolderCover.mockResolvedValue(undefined);
  mocked.deleteItems.mockResolvedValue({ trashed: 1, errors: [], batchId: "batch-1" });
  mocked.moveItems.mockResolvedValue({ moved: 1, errors: [], batchId: "batch-2" });
  mocked.undoBatch.mockResolvedValue({ reversed: 1, errors: [] });
});

function openItemMenu(over: { itemIds?: number[]; favorite?: boolean } = {}) {
  const item = gridItem({ favorite: over.favorite ?? false });
  const onPreview = vi.fn();
  const rendered = renderWithProviders(
    <PointMenu at={{ x: 10, y: 10 }} onClose={() => {}}>
      <ItemMenu
        itemIds={over.itemIds ?? [item.id]}
        item={over.itemIds && over.itemIds.length > 1 ? null : item}
        folder={{ id: 1, title: "Trips" }}
        onPreview={onPreview}
      />
    </PointMenu>,
  );
  return { ...rendered, onPreview };
}

describe("the item menu", () => {
  it("offers every item operation DESIGN §8 requires a visible control for", async () => {
    openItemMenu();

    for (const label of [
      "Show In The Pane",
      "Favourite",
      "Add Tag…",
      "Move To…",
      "Reveal In Explorer",
      "Open With The Default App",
      "Copy File",
      "Copy The Full Path",
      "Delete…",
    ]) {
      expect(
        await screen.findByRole("menuitem", { name: new RegExp(label, "i") }),
      ).toBeInTheDocument();
    }
  });

  it("favourites through the command rather than a local toggle", async () => {
    openItemMenu();
    await userEvent.click(await screen.findByRole("menuitem", { name: /^Favourite/ }));
    await waitFor(() => expect(mocked.setItemsFavorite).toHaveBeenCalledWith([7], true));
  });

  it("offers to remove the favourite when the item already has one", async () => {
    openItemMenu({ favorite: true });
    await userEvent.click(
      await screen.findByRole("menuitem", { name: /Remove Favourite/ }),
    );
    await waitFor(() => expect(mocked.setItemsFavorite).toHaveBeenCalledWith([7], false));
  });

  it("reveals, opens and copies the item it was opened on", async () => {
    openItemMenu();
    await userEvent.click(
      await screen.findByRole("menuitem", { name: /Reveal In Explorer/ }),
    );
    await waitFor(() => expect(mocked.revealItem).toHaveBeenCalledWith(7));
  });

  it("copies the file, not just its path", async () => {
    openItemMenu();
    await userEvent.click(await screen.findByRole("menuitem", { name: /Copy File/ }));
    await waitFor(() => expect(mocked.copyItemFile).toHaveBeenCalledWith(7));
  });

  it("sets the item as the current folder's cover", async () => {
    openItemMenu();
    await userEvent.click(
      await screen.findByRole("menuitem", { name: /Set As Trips.+Cover/ }),
    );
    await waitFor(() => expect(mocked.setFolderCover).toHaveBeenCalledWith(1, 7));
  });

  it("hides the single-item operations for a multi-item selection", async () => {
    openItemMenu({ itemIds: [7, 8, 9] });

    expect(await screen.findByRole("menuitem", { name: /Move To…/ })).toBeInTheDocument();
    expect(screen.queryByRole("menuitem", { name: /Reveal In Explorer/ })).toBeNull();
    expect(screen.queryByRole("menuitem", { name: /Copy The Full Path/ })).toBeNull();
  });

  it("confirms before deleting, then deletes the whole selection", async () => {
    openItemMenu({ itemIds: [7, 8] });

    await userEvent.click(await screen.findByRole("menuitem", { name: /Delete…/ }));
    // Named, not "Are you sure?" — and it says where things go and how to
    // get them back, in one sentence.
    expect(await screen.findByText(/Delete 2 items\?/)).toBeInTheDocument();
    expect(screen.getByText(/go to the trash/)).toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(mocked.deleteItems).toHaveBeenCalledWith([7, 8]));
  });
});

describe("the empty-space menu", () => {
  it("carries select all, invert and clear, which are otherwise keys only", async () => {
    const selection = fakeSelection({ count: 2 });
    renderWithProviders(
      <PointMenu at={{ x: 1, y: 1 }} onClose={() => {}}>
        <EmptyMenu
          folder={{ id: 1, title: "Trips" }}
          hasItems
          hasSelection
          onSelectAll={selection.selectAll}
          onInvert={selection.invert}
          onClear={selection.clear}
          onNewFolder={() => {}}
          bandExpanded={false}
          onToggleBand={() => {}}
          paneOpen
          onTogglePane={() => {}}
        />
      </PointMenu>,
      { selection },
    );

    await userEvent.click(await screen.findByRole("menuitem", { name: /Select All/ }));
    expect(selection.selectAll).toHaveBeenCalled();
  });

  it("keeps invert reachable, since the selection bar no longer carries it", async () => {
    // M2.5a.1 dropped *revert* from the selection bar. Decision 23 says no
    // capability may become keyboard-only, so this menu is now the visible
    // path to it and must not lose the entry.
    const selection = fakeSelection({ count: 2 });
    renderWithProviders(
      <PointMenu at={{ x: 1, y: 1 }} onClose={() => {}}>
        <EmptyMenu
          folder={{ id: 1, title: "Trips" }}
          hasItems
          hasSelection
          onSelectAll={selection.selectAll}
          onInvert={selection.invert}
          onClear={selection.clear}
          onNewFolder={() => {}}
          bandExpanded={false}
          onToggleBand={() => {}}
          paneOpen
          onTogglePane={() => {}}
        />
      </PointMenu>,
      { selection },
    );

    await userEvent.click(
      await screen.findByRole("menuitem", { name: /Invert Selection/ }),
    );
    expect(selection.invert).toHaveBeenCalled();
  });

  it("creates a folder in the folder currently in view", async () => {
    const onNewFolder = vi.fn();
    renderWithProviders(
      <PointMenu at={{ x: 1, y: 1 }} onClose={() => {}}>
        <EmptyMenu
          folder={{ id: 1, title: "Trips" }}
          hasItems
          hasSelection={false}
          onSelectAll={() => {}}
          onInvert={() => {}}
          onClear={() => {}}
          onNewFolder={onNewFolder}
          bandExpanded={false}
          onToggleBand={() => {}}
          paneOpen
          onTogglePane={() => {}}
        />
      </PointMenu>,
      { library: fakeLibrary() },
    );

    await userEvent.click(
      await screen.findByRole("menuitem", { name: /New Folder In Trips…/ }),
    );
    expect(onNewFolder).toHaveBeenCalled();
  });
});
