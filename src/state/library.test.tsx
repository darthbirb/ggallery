/**
 * The startup flow's exit conditions.
 *
 * The regression these exist for: the Progress screen used to wait for a
 * *busy* progress event followed by an *idle* one. Progress events are emitted
 * only when the numbers change, so a library small enough to finish indexing
 * before the screen appeared emitted its last event with nobody listening and
 * then went quiet — and the screen waited forever for an event that was never
 * coming. Twenty-three files was small enough.
 */

import { renderHook, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Progress } from "../lib/types";
import { libraryInfo } from "../test/harness";
import { useLibrary } from "./library";

vi.mock("../lib/ipc");

import * as ipc from "../lib/ipc";

const mocked = vi.mocked(ipc);

function progress(over: Partial<Progress> = {}): Progress {
  return {
    phase: "idle",
    itemsChecked: 23,
    queued: 0,
    items: 23,
    pending: 0,
    running: 0,
    failed: 0,
    completed: 23,
    lastError: null,
    rescanning: false,
    ...over,
  };
}

beforeEach(() => {
  mocked.errorMessage.mockImplementation((err: unknown) => String(err));
  mocked.currentLibrary.mockResolvedValue({ info: null, remembered: null });
  mocked.onProgress.mockResolvedValue(() => {});
  mocked.onImportProgress.mockResolvedValue(() => {});
  mocked.listItems.mockResolvedValue([]);
  mocked.folderTree.mockResolvedValue([]);
  mocked.indexFailures.mockResolvedValue([]);
  mocked.startIndex.mockResolvedValue(undefined);
  mocked.cancelPreparedImport.mockResolvedValue(undefined);
  mocked.executePreparedImport.mockResolvedValue({ moved: 23, folders: 4, errors: [] });
  // A fresh import: nothing was indexed at the moment the library opened.
  mocked.openLibrary.mockResolvedValue({ ...libraryInfo(), itemCount: 0 });
});

/** Drive the hook from "a folder was picked" to the Progress screen. */
async function reachIndexing() {
  mocked.prepareImport.mockResolvedValue({
    alreadyImported: false,
    byKind: [],
    totalItems: 23,
    totalBytes: 1000,
    folderCount: 4,
    unreadable: 0,
  });

  const { result } = renderHook(() => useLibrary());
  await waitFor(() => expect(result.current.loading).toBe(false));

  result.current.open("D:/Pictures");
  await waitFor(() => expect(result.current.pendingReview).not.toBeNull());

  result.current.confirmImport(true);
  // The flow is asynchronous, and `flowPhase` is "idle" both before it starts
  // and after it finishes — so every assertion below waits for evidence the
  // import actually ran rather than for a value that was already true.
  await waitFor(() => expect(mocked.executePreparedImport).toHaveBeenCalled());
  return result;
}

describe("the import flow's Progress screen", () => {
  it("finishes when the queue drained before anyone was listening", async () => {
    // The whole bug in one line: the queue is already idle by the time the
    // screen appears, and no further event will ever be emitted.
    mocked.indexProgress.mockResolvedValue(progress({ phase: "idle" }));

    const result = await reachIndexing();

    await waitFor(() => expect(result.current.flowPhase).toBe("idle"), {
      timeout: 3000,
    });
  });

  it("stays up while the queue is still working, then leaves", async () => {
    mocked.indexProgress
      .mockResolvedValueOnce(progress({ phase: "walking", items: 0 }))
      .mockResolvedValueOnce(progress({ phase: "working", pending: 4, items: 19 }))
      .mockResolvedValue(progress({ phase: "idle" }));

    const result = await reachIndexing();

    await waitFor(() => expect(result.current.flowPhase).toBe("indexing"));
    await waitFor(() => expect(result.current.flowPhase).toBe("idle"), {
      timeout: 5000,
    });
  });

  it("does not strand the user when the queue cannot be read at all", async () => {
    mocked.indexProgress.mockRejectedValue(new Error("library closed"));

    const result = await reachIndexing();

    await waitFor(() => expect(result.current.info).not.toBeNull(), { timeout: 3000 });
    await waitFor(() => expect(result.current.flowPhase).toBe("idle"));
  });
});

describe("opening a library that needs no import", () => {
  it("goes straight to the gallery", async () => {
    mocked.prepareImport.mockResolvedValue({
      alreadyImported: true,
      byKind: [],
      totalItems: 0,
      totalBytes: 0,
      folderCount: 0,
      unreadable: 0,
    });
    mocked.indexProgress.mockResolvedValue(progress());

    const { result } = renderHook(() => useLibrary());
    await waitFor(() => expect(result.current.loading).toBe(false));

    result.current.open("D:/Pictures");

    await waitFor(() => expect(result.current.info).not.toBeNull());
    expect(result.current.pendingReview).toBeNull();
    expect(result.current.flowPhase).toBe("idle");
  });
});
