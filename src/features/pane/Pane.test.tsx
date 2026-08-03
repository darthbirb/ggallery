/**
 * The pane's header.
 *
 * It carries the item's identity, not a mode label. M2.5a shipped a tablist
 * whose only tab said "Preview" — a label wearing a control's clothes — and
 * before that two more that were disabled. M2.5b brings a real switcher back
 * to the same slot once there is something to switch between; until then the
 * header names what you are looking at, and the window controls stay put.
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
  it("names the item rather than the mode", async () => {
    renderPane();
    expect(await screen.findByText("beach.jpg")).toBeInTheDocument();
    expect(screen.queryByRole("tab")).toBeNull();
    expect(screen.queryByText("Preview")).toBeNull();
  });

  it("keeps maximise and close as labelled controls", () => {
    renderPane();
    expect(screen.getByRole("button", { name: "Fill the window" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close the pane" })).toBeInTheDocument();
  });

  it("still shows the window controls with nothing selected", () => {
    renderPane({ slots: [{ key: "primary", itemId: null }] });
    expect(screen.getByText("Nothing selected.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Close the pane" })).toBeInTheDocument();
  });
});
