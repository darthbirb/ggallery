/**
 * Preview mode is built for N panes from the start — it is one component
 * serving M2.5a, M6, M7 and M10, and "we'll generalise it later" is how you
 * end up with a single-pane viewer plus three bespoke comparison screens.
 *
 * These tests hold that open: the layout is exercised at every size
 * docs/DESIGN.md §2 names, and the component is rendered with more than one
 * slot even though nothing in M2.5a passes more than one yet.
 */

import { fireEvent, screen, waitFor } from "@testing-library/react";
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
      filmstripHeight={64}
      onFilmstripHeightChange={vi.fn()}
      onResetFilmstripHeight={vi.fn()}
      mode="preview"
      onModeChange={vi.fn()}
      maximised={false}
      onMaximisedChange={vi.fn()}
      onClose={vi.fn()}
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

  it("at fit, has no zoom UI at all — scroll and drag are the whole interaction", async () => {
    renderPreview();
    await screen.findByAltText("beach.jpg");
    // DESIGN.md §2: no fit button, no 1:1 button, no percentage readout.
    expect(screen.queryByRole("button", { name: /^Fit$/ })).toBeNull();
    expect(screen.queryByRole("button", { name: "1:1" })).toBeNull();
    expect(screen.queryByText("fit")).toBeNull();
    expect(screen.queryByRole("button", { name: /^Zoom/ })).toBeNull();
  });

  it("once zoom leaves fit, shows a percentage readout that returns to fit on click", async () => {
    renderPreview();
    const image = await screen.findByAltText("beach.jpg");

    fireEvent.wheel(image, { deltaY: -100 });

    const readout = await screen.findByRole("button", {
      name: /^Zoom \d+% — click to fit$/,
    });
    await userEvent.click(readout);
    expect(screen.queryByRole("button", { name: /^Zoom/ })).toBeNull();
  });

  it("opens the details from the header, downward, above the media", async () => {
    const { rerender } = renderPreview({ detailsExpanded: true });
    const toggle = await screen.findByRole("button", { expanded: true });
    const created = await screen.findByText("Created");
    const strip = await screen.findByRole("button", { name: "cliff.jpg" });

    // Header, then the details body, then the media, then the strip. The
    // strip is last in document order, which is what keeps it pinned to the
    // bottom edge while the details push the media down.
    expect(
      toggle.compareDocumentPosition(created) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(
      created.compareDocumentPosition(strip) & Node.DOCUMENT_POSITION_FOLLOWING,
    ).toBeTruthy();
    expect(rerender).toBeTruthy();
  });

  it("keeps the filmstrip's height under the caller's control", async () => {
    renderPreview({ filmstripHeight: 140 });
    const handle = await screen.findByRole("separator", { name: "Filmstrip height" });
    expect(handle).toHaveAttribute("aria-valuenow", "140");
    expect(handle).toHaveAttribute("aria-orientation", "horizontal");
  });
});

describe("details", () => {
  it("collapsed, shows dimensions and size only — the name moved to the expanded body", async () => {
    renderPreview();
    expect(await screen.findByText(/1200×800/)).toBeInTheDocument();
    expect(screen.queryByText("beach.jpg")).toBeNull();
    expect(screen.queryByText("Created")).toBeNull();
  });

  it("expanded, adds the name, the dates, the source and the tags", async () => {
    mocked.itemEffectiveTags.mockResolvedValue([
      { tagId: 1, key: null, value: "beach", originId: null },
      // The folder's own auto title-tag, inherited — same as "Trips" showing
      // up as a real effective tag on every item inside it.
      { tagId: 2, key: null, value: "Trips", originId: 5 },
    ]);
    renderPreview({ detailsExpanded: true });

    expect(await screen.findByText("beach.jpg")).toBeInTheDocument();
    expect(await screen.findByText("Created")).toBeInTheDocument();
    expect(await screen.findByText("beach")).toBeInTheDocument();

    // A manual tag can be removed here; an inherited one cannot — it comes
    // from the folder, and that is where it changes.
    expect(screen.getByRole("button", { name: "Remove beach" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Remove Trips" })).toBeNull();

    // "Trips" is the folder itself — it reads once, as a breadcrumb crumb,
    // not a second time as a tag-shaped chip repeating the same fact.
    expect(screen.getAllByText("Trips")).toHaveLength(1);
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
