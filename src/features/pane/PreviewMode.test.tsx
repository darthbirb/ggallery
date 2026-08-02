/**
 * Preview mode is built for N panes from the start — it is one component
 * serving M2.5a, M6, M7 and M10, and "we'll generalise it later" is how you
 * end up with a single-pane viewer plus three bespoke comparison screens.
 *
 * These tests hold that open: the layout is exercised at every size
 * docs/DESIGN.md §2 names, and the component is rendered with more than one
 * slot even though nothing in M2.5a passes more than one yet.
 */

import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { fakeLibrary, gridItem, itemDetail, renderWithProviders } from "../../test/harness";
import { PreviewMode, paneGrid } from "./PreviewMode";

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

describe("the pane layout", () => {
  it("matches the shape DESIGN §2 specifies at every size", () => {
    // 2 side by side, 3–4 as 2×2, 5–6 as 3×2, 7–9 as 3×3, 10–12 as 4×3.
    expect(paneGrid(1)).toEqual({ columns: 1, rows: 1 });
    expect(paneGrid(2)).toEqual({ columns: 2, rows: 1 });
    expect(paneGrid(3)).toEqual({ columns: 2, rows: 2 });
    expect(paneGrid(4)).toEqual({ columns: 2, rows: 2 });
    expect(paneGrid(6)).toEqual({ columns: 3, rows: 2 });
    expect(paneGrid(9)).toEqual({ columns: 3, rows: 3 });
    expect(paneGrid(12)).toEqual({ columns: 4, rows: 3 });
  });
});

function renderPreview(over: Partial<React.ComponentProps<typeof PreviewMode>> = {}) {
  const onStep = vi.fn();
  const onPick = vi.fn();
  const items = [
    gridItem({ id: 7, name: "beach.jpg" }),
    gridItem({ id: 8, name: "cliff.jpg" }),
  ];
  const result = renderWithProviders(
    <PreviewMode
      slots={[{ key: "primary", itemId: 7 }]}
      items={items}
      thumbsDir="D:/thumbs"
      onStep={onStep}
      onPick={onPick}
      detailsExpanded={false}
      onDetailsExpandedChange={vi.fn()}
      refreshToken={0}
      {...over}
    />,
    { library: fakeLibrary({ items }) },
  );
  return { ...result, onStep, onPick, items };
}

describe("preview", () => {
  it("shows an empty state when nothing is selected", () => {
    renderPreview({ slots: [{ key: "primary", itemId: null }] });
    expect(screen.getByText(/Nothing selected/)).toBeInTheDocument();
  });

  it("loads the item it was given", async () => {
    renderPreview();
    await waitFor(() => expect(mocked.getItem).toHaveBeenCalledWith(7));
  });

  it("renders one view per slot, so two panes is two items", async () => {
    mocked.getItem.mockImplementation(async (id: number) =>
      itemDetail({ id, origName: `item-${id}.jpg` }),
    );
    renderPreview({
      slots: [
        { key: "a", itemId: 7 },
        { key: "b", itemId: 8 },
      ],
    });

    await waitFor(() => expect(mocked.getItem).toHaveBeenCalledWith(7));
    expect(mocked.getItem).toHaveBeenCalledWith(8);
    // Two item views, side by side. The filmstrip's thumbnails are
    // decorative (empty alt) and are deliberately not counted here.
    expect(await screen.findByAltText("item-7.jpg")).toBeInTheDocument();
    expect(await screen.findByAltText("item-8.jpg")).toBeInTheDocument();
  });

  it("moves through the current filter with the chevrons", async () => {
    const { onStep } = renderPreview();
    await userEvent.click(await screen.findByRole("button", { name: "Next" }));
    expect(onStep).toHaveBeenCalledWith(1);
  });

  it("jumps straight to an item from the filmstrip", async () => {
    const { onPick } = renderPreview();
    await userEvent.click(await screen.findByRole("button", { name: "cliff.jpg" }));
    expect(onPick).toHaveBeenCalledWith(8);
  });

  it("says where in the filter the current item is", async () => {
    renderPreview();
    expect(await screen.findByText("1/2")).toBeInTheDocument();
  });
});

describe("details", () => {
  it("collapsed, shows the filename, dimensions and size only", async () => {
    renderPreview();
    expect(await screen.findByText("beach.jpg")).toBeInTheDocument();
    expect(screen.getByText(/1200×800/)).toBeInTheDocument();
    expect(screen.queryByText("Captured")).toBeNull();
  });

  it("expanded, adds the dates, the source and the tags", async () => {
    mocked.itemEffectiveTags.mockResolvedValue([
      { tagId: 1, key: null, value: "beach", originId: null },
      { tagId: 2, key: null, value: "Trips", originId: 5 },
    ]);
    renderPreview({ detailsExpanded: true });

    expect(await screen.findByText("Captured")).toBeInTheDocument();
    expect(await screen.findByText("beach")).toBeInTheDocument();

    // A manual tag can be removed here; an inherited one cannot — it comes
    // from the folder, and that is where it changes.
    expect(screen.getByRole("button", { name: "Remove beach" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove Trips" })).toBeNull();
  });

  it("adds a tag to the item from inside the details", async () => {
    mocked.addItemTag.mockResolvedValue(undefined);
    renderPreview({ detailsExpanded: true });

    await userEvent.type(
      await screen.findByLabelText("Add a tag to this item"),
      "blurry{Enter}",
    );

    await waitFor(() => expect(mocked.addItemTag).toHaveBeenCalledWith(7, null, "blurry"));
  });

  it("reads key: value as a label", async () => {
    mocked.addItemTag.mockResolvedValue(undefined);
    renderPreview({ detailsExpanded: true });

    await userEvent.type(
      await screen.findByLabelText("Add a tag to this item"),
      "city: lisbon{Enter}",
    );

    await waitFor(() =>
      expect(mocked.addItemTag).toHaveBeenCalledWith(7, "city", "lisbon"),
    );
  });
});
