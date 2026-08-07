/**
 * The pane's header.
 *
 * It carries dimensions and size, not a mode label or the filename — the
 * filename moved into the expanded details body to leave room for the
 * header's own fold and mode controls. M2.5a shipped a tablist whose only
 * tab said "Preview" — a label wearing a control's clothes — and before that
 * two more that were disabled; the mode buttons here are the real thing, and
 * as of M2.5b all three actually switch modes.
 */

import { fireEvent, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, folderDetail, gridItem, itemDetail, renderWithProviders } from "../../test/harness";
import { Pane } from "./Pane";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.assetUrl.mockReturnValue("asset://thumb");
  mocked.assetPath.mockReturnValue("asset://file");
  mocked.getItem.mockResolvedValue(itemDetail());
  mocked.itemEffectiveTags.mockResolvedValue([]);
  mocked.listItems.mockResolvedValue([]);
  mocked.getFolder.mockResolvedValue(folderDetail());
});

function renderPane(over: Partial<React.ComponentProps<typeof Pane>> = {}) {
  const items = [gridItem({ id: 7, name: "beach.jpg" })];
  return renderWithProviders(
    <Pane
      mode="preview"
      onModeChange={vi.fn()}
      onClose={vi.fn()}
      maximised={false}
      onMaximisedChange={vi.fn()}
      slots={[{ key: "primary", itemId: 7 }]}
      items={items}
      thumbsDir="D:/thumbs"
      spritesDir="D:/sprites"
      onStep={vi.fn()}
      onPick={vi.fn()}
      detailsExpanded={false}
      onDetailsExpandedChange={vi.fn()}
      filmstripHeight={64}
      onFilmstripHeightChange={vi.fn()}
      onResetFilmstripHeight={vi.fn()}
      refreshToken={0}
      folders={[]}
      onOpenInMain={vi.fn()}
      {...over}
    />,
    { library: fakeLibrary({ items }) },
  );
}

describe("the pane header", () => {
  it("shows dimensions and size rather than a mode label", async () => {
    renderPane();
    expect(await screen.findByText(/1200×800/)).toBeInTheDocument();
    expect(screen.queryByRole("tab")).toBeNull();
    expect(screen.queryByText("Preview")).toBeNull();
  });

  it("keeps maximise, fold and the mode switcher as labelled controls", () => {
    renderPane();
    expect(screen.getByRole("button", { name: "Fill The Window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide The Pane" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Grid" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Folders" })).toBeInTheDocument();
  });

  it("still shows the window controls with nothing selected", () => {
    renderPane({ slots: [{ key: "primary", itemId: null }] });
    expect(screen.getByText("Nothing Selected")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide The Pane" })).toBeInTheDocument();
  });

  it("switches to Grid mode, which renders its own frame rather than staying inert", () => {
    const onModeChange = vi.fn();
    renderPane({ onModeChange });
    fireEvent.click(screen.getByRole("button", { name: "Grid" }));
    expect(onModeChange).toHaveBeenCalledWith("grid");
  });

  it("switches to Folders mode, which renders its own frame rather than staying inert", () => {
    const onModeChange = vi.fn();
    renderPane({ onModeChange });
    fireEvent.click(screen.getByRole("button", { name: "Folders" }));
    expect(onModeChange).toHaveBeenCalledWith("folders");
  });
});

describe("Grid and Folders modes actually render", () => {
  it("Grid mode shows an empty state rather than nothing", async () => {
    renderPane({ mode: "grid" });
    expect(await screen.findByText("Nothing Here Yet")).toBeInTheDocument();
  });

  it("Folders mode shows the top-level tree with a New folder tile", async () => {
    renderPane({
      mode: "folders",
      folders: [
        {
          id: 1,
          title: "trips",
          parentId: null,
          depth: 0,
          directCount: 0,
          totalCount: 3,
          status: "active",
          favorite: false,
        },
      ],
    });
    expect(await screen.findByText("trips")).toBeInTheDocument();
    expect(screen.getByText(/New Folder In/)).toBeInTheDocument();
  });
});
