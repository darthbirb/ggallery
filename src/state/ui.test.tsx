/**
 * Interface preferences: one accent on the root, and panel state that
 * survives a restart. Locked decision 24, and docs/DESIGN.md §2 "Panels are
 * resizable".
 */

import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { NAV_DEFAULT, PANE_DEFAULT, PANE_MIN, UiProvider, useUi } from "./ui";

vi.mock("../lib/ipc");

import * as ipc from "../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  vi.useRealTimers();
  mocked.uiPrefs.mockResolvedValue(null);
  mocked.setUiPrefs.mockResolvedValue(undefined);
  document.documentElement.removeAttribute("data-accent");
});

function Probe() {
  const ui = useUi();
  return (
    <>
      <span data-testid="accent">{ui.accent}</span>
      <span data-testid="nav-width">{ui.navWidth}</span>
      <span data-testid="pane-width">{ui.paneWidth}</span>
      <span data-testid="band">{String(ui.bandExpanded)}</span>
      <button onClick={() => ui.set("accent", "teal")}>teal</button>
      <button onClick={() => ui.set("navWidth", 260)}>widen</button>
      <button onClick={() => ui.set("paneMode", "grid")}>grid mode</button>
      <button onClick={() => ui.setPaneWidth(40)}>crush pane</button>
    </>
  );
}

function renderUi() {
  return render(
    <UiProvider>
      <Probe />
    </UiProvider>,
  );
}

describe("the accent", () => {
  it("defaults to Slate and lands on the root as data-accent", async () => {
    renderUi();
    await waitFor(() =>
      expect(document.documentElement.dataset.accent).toBe("slate"),
    );
    expect(screen.getByTestId("accent")).toHaveTextContent("slate");
  });

  it("swaps wholesale when another is chosen", async () => {
    renderUi();
    await userEvent.click(screen.getByText("teal"));
    await waitFor(() => expect(document.documentElement.dataset.accent).toBe("teal"));
  });

  it("comes back from stored preferences", async () => {
    mocked.uiPrefs.mockResolvedValue({ accent: "violet" });
    renderUi();
    await waitFor(() => expect(document.documentElement.dataset.accent).toBe("violet"));
  });

  it("ignores an accent that is not in the fixed set", async () => {
    mocked.uiPrefs.mockResolvedValue({ accent: "chartreuse" });
    renderUi();
    await waitFor(() => expect(screen.getByTestId("accent")).toHaveTextContent("slate"));
  });
});

describe("panel state", () => {
  it("starts at the defaults when nothing is stored", async () => {
    renderUi();
    await waitFor(() =>
      expect(screen.getByTestId("nav-width")).toHaveTextContent(String(NAV_DEFAULT)),
    );
    expect(screen.getByTestId("pane-width")).toHaveTextContent(String(PANE_DEFAULT));
  });

  it("keeps one pane width, whatever mode the pane is in", async () => {
    // It was remembered per mode until M2.5a.1. Switching modes moving the
    // split under you reads as the window losing its place.
    mocked.uiPrefs.mockResolvedValue({ paneMode: "preview", paneWidth: 400 });
    renderUi();

    await waitFor(() =>
      expect(screen.getByTestId("pane-width")).toHaveTextContent("400"),
    );
    await userEvent.click(screen.getByText("grid mode"));
    expect(screen.getByTestId("pane-width")).toHaveTextContent("400");
  });

  it("carries a pre-M2.5a.1 file's Preview width over to the single width", async () => {
    // The only mode ever built, so it is the one that was actually dragged —
    // taking it is what stops the split jumping on upgrade.
    mocked.uiPrefs.mockResolvedValue({
      paneWidths: { preview: 512, grid: 700, folders: 300 },
    });
    renderUi();
    await waitFor(() =>
      expect(screen.getByTestId("pane-width")).toHaveTextContent("512"),
    );
  });

  it("refuses a width the pane could not render at", async () => {
    renderUi();
    await waitFor(() => expect(mocked.uiPrefs).toHaveBeenCalled());
    await userEvent.click(screen.getByText("crush pane"));
    expect(screen.getByTestId("pane-width")).toHaveTextContent(String(PANE_MIN));
  });

  it("persists a change, debounced, next to the exe", async () => {
    renderUi();
    await waitFor(() => expect(mocked.uiPrefs).toHaveBeenCalled());

    await userEvent.click(screen.getByText("widen"));

    await waitFor(
      () =>
        expect(mocked.setUiPrefs).toHaveBeenCalledWith(
          expect.objectContaining({ navWidth: 260 }),
        ),
      { timeout: 2000 },
    );
  });

  it("never writes before it has read, so a slow read cannot clobber the file", async () => {
    let release: (value: unknown) => void = () => {};
    mocked.uiPrefs.mockReturnValue(
      new Promise((resolve) => {
        release = resolve;
      }),
    );

    renderUi();
    await new Promise((resolve) => setTimeout(resolve, 600));
    expect(mocked.setUiPrefs).not.toHaveBeenCalled();

    await act(async () => {
      release({ navWidth: 300 });
    });
    await waitFor(() =>
      expect(screen.getByTestId("nav-width")).toHaveTextContent("300"),
    );
  });

  it("falls back to a usable interface when the preferences cannot be read", async () => {
    mocked.uiPrefs.mockRejectedValue(new Error("no config"));
    renderUi();
    await waitFor(() =>
      expect(screen.getByTestId("nav-width")).toHaveTextContent(String(NAV_DEFAULT)),
    );
  });

  it("keeps the band's expanded state global rather than per folder", async () => {
    mocked.uiPrefs.mockResolvedValue({ bandExpanded: true });
    renderUi();
    // One value, stored once — there is nowhere to put a per-folder one, by
    // design: it would reflow the grid on every navigation.
    await waitFor(() => expect(screen.getByTestId("band")).toHaveTextContent("true"));
  });
});
