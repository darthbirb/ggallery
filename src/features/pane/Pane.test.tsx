/**
 * The pane's header.
 *
 * It carries dimensions and size, not a mode label or the filename — the
 * filename moved into the expanded details body to leave room for the
 * header's own fold and mode controls. M2.5a shipped a tablist whose only
 * tab said "Preview" — a label wearing a control's clothes — and before that
 * two more that were disabled; the mode buttons here are the real thing,
 * with Grid and Folders inert until M2.5b builds them.
 */

import { screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, gridItem, itemDetail, renderWithProviders } from "../../test/harness";
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
      onStep={vi.fn()}
      onPick={vi.fn()}
      detailsExpanded={false}
      onDetailsExpandedChange={vi.fn()}
      filmstripHeight={64}
      onFilmstripHeightChange={vi.fn()}
      onResetFilmstripHeight={vi.fn()}
      refreshToken={0}
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
    expect(screen.getByRole("button", { name: "Fill the window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide the pane" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Grid" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Folders" })).toBeInTheDocument();
  });

  it("still shows the window controls with nothing selected", () => {
    renderPane({ slots: [{ key: "primary", itemId: null }] });
    expect(screen.getByText("Nothing selected.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Hide the pane" })).toBeInTheDocument();
  });
});
