/**
 * Toast-and-undo, which is not decoration: M2.1 shipped journalled moves and
 * deletes with no mouse path to reverse them, and this is the path. Locked
 * decision 23.
 *
 * What these tests hold in place: every destructive operation ends in a toast,
 * the toast names what happened, and its Undo button reverses the batch the
 * operation actually wrote — not some other one.
 */

import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";

import { Toaster, ToastProviderRoot } from "../../components/Toaster";
import { fakeLibrary, fakeSelection, folderNode } from "../../test/harness";
import { ToastProvider } from "../../state/toasts";
import { OperationsProvider, useOperations } from "./operations";

vi.mock("../../lib/ipc");

import * as ipc from "../../lib/ipc";

const mocked = vi.mocked(ipc);

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) =>
    err instanceof Error ? err.message : String(err),
  );
  mocked.undoBatch.mockResolvedValue({ reversed: 2, errors: [] });
});

/** A button per operation, so a click is the whole test. */
function Harness() {
  const ops = useOperations();
  return (
    <>
      <button onClick={() => void ops.deleteItems([7, 8])}>delete</button>
      <button onClick={() => void ops.moveItems([7, 8], folderNode())}>move</button>
      <button onClick={() => void ops.deleteFolder({ id: 1, title: "Trips" })}>
        delete folder
      </button>
      <button onClick={() => void ops.renameFolder({ id: 1, title: "Trips" }, "Journeys")}>
        rename folder
      </button>
      <button onClick={() => void ops.copyItemPath(7)}>copy path</button>
    </>
  );
}

function setup() {
  const library = fakeLibrary();
  const selection = fakeSelection();
  render(
    <ToastProvider>
      <ToastProviderRoot>
        <OperationsProvider library={library} selection={selection}>
          <Harness />
        </OperationsProvider>
        <Toaster />
      </ToastProviderRoot>
    </ToastProvider>,
  );
  return { library, selection };
}

describe("destructive operations", () => {
  it("delete ends in a toast naming what happened, with Undo", async () => {
    mocked.deleteItems.mockResolvedValue({
      trashed: 2,
      errors: [],
      batchId: "batch-delete",
    });
    setup();

    await userEvent.click(screen.getByText("delete"));

    expect(await screen.findByText("Deleted 2 Items")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));

    await waitFor(() => expect(mocked.undoBatch).toHaveBeenCalledWith("batch-delete"));
    expect(await screen.findByText("Restored 2 Items")).toBeInTheDocument();
  });

  it("move names the destination and undoes its own batch", async () => {
    mocked.moveItems.mockResolvedValue({ moved: 2, errors: [], batchId: "batch-move" });
    setup();

    await userEvent.click(screen.getByText("move"));

    expect(await screen.findByText("Moved 2 Items To Trips")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(mocked.undoBatch).toHaveBeenCalledWith("batch-move"));
  });

  it("deleting a folder is undoable too", async () => {
    mocked.deleteFolder.mockResolvedValue("batch-folder");
    setup();

    await userEvent.click(screen.getByText("delete folder"));

    expect(
      await screen.findByText("Deleted Trips"),
    ).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(mocked.undoBatch).toHaveBeenCalledWith("batch-folder"));
  });

  it("renaming a folder is undoable, and says both names", async () => {
    mocked.setFolderTitle.mockResolvedValue("batch-rename");
    setup();

    await userEvent.click(screen.getByText("rename folder"));

    expect(await screen.findByText("Renamed Trips To Journeys")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Undo" }));
    await waitFor(() => expect(mocked.undoBatch).toHaveBeenCalledWith("batch-rename"));
  });

  it("reports a failed undo in the same toast rather than losing it", async () => {
    mocked.deleteItems.mockResolvedValue({
      trashed: 1,
      errors: [],
      batchId: "batch-x",
    });
    mocked.undoBatch.mockResolvedValue({
      reversed: 0,
      errors: ["something already exists at trips/a.jpg"],
    });
    setup();

    await userEvent.click(screen.getByText("delete"));
    await userEvent.click(await screen.findByRole("button", { name: "Undo" }));

    expect(await screen.findByText(/Could not undo: something already exists/))
      .toBeInTheDocument();
  });

  it("reloads the view once an operation lands, so counts agree again", async () => {
    mocked.deleteItems.mockResolvedValue({ trashed: 1, errors: [], batchId: "b" });
    const { library, selection } = setup();

    await userEvent.click(screen.getByText("delete"));

    await waitFor(() => expect(library.reload).toHaveBeenCalled());
    expect(selection.clear).toHaveBeenCalled();
  });
});

describe("non-destructive operations", () => {
  it("confirm in a toast without offering an undo", async () => {
    mocked.copyItemPath.mockResolvedValue(undefined);
    setup();

    await userEvent.click(screen.getByText("copy path"));

    expect(await screen.findByText("Path Copied")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Undo" })).toBeNull();
  });

  it("surface a failure as a toast rather than a silent no-op", async () => {
    mocked.deleteItems.mockRejectedValue(new Error("the file is locked"));
    setup();

    await userEvent.click(screen.getByText("delete"));

    expect(
      await screen.findByText("Could not delete: the file is locked"),
    ).toBeInTheDocument();
  });
});
