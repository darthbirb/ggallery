import { useCallback, useEffect, useRef, useState } from "react";

import * as ipc from "../lib/ipc";
import type {
  FolderNode,
  FsExecuteReport,
  GridItem,
  ImportProgress,
  IndexFailure,
  LibraryInfo,
  Progress,
  ReviewReport,
  VerifyReport,
} from "../lib/types";

/**
 * The three navigation roots docs/DESIGN.md §2 requires are distinct things,
 * not three shapes of one folder query: *Everything* ignores folder structure,
 * *Loose items* is the top level and nothing beneath it, and a folder is a
 * folder. Favourites is a fourth root, filtered from Everything.
 *
 * The library root is never presented as a folder — see `features/nav`.
 */
export type ViewKind = "everything" | "loose" | "favourites" | "folder";

export interface Scope {
  kind: ViewKind;
  /** Folder rel_path when `kind` is "folder"; null otherwise. */
  folder: string | null;
  /** Folder views are recursive by default — PLAN.md decision 10. */
  recursive: boolean;
}

export const EVERYTHING: Scope = {
  kind: "everything",
  folder: null,
  recursive: true,
};

/** What `listItems` is asked for, per root. */
function query(scope: Scope): { folder: string | null; recursive: boolean } {
  switch (scope.kind) {
    case "everything":
    case "favourites":
      return { folder: null, recursive: true };
    case "loose":
      // `Some("")` and non-recursive: the root folder's own items. Distinct
      // from `None`, which is the whole library.
      return { folder: "", recursive: false };
    case "folder":
      return { folder: scope.folder, recursive: scope.recursive };
  }
}

/** What a library not yet imported is waiting on — the Review screen's
 *  whole content, plus which folder it is for. */
export interface PendingReview {
  path: string;
  report: ReviewReport;
}

/** Where the M1.7 startup flow is, once a folder has been chosen and it
 *  turned out to need the import ceremony. `"idle"` covers everything
 *  else — nothing pending, or an already-imported library opening normally. */
export type FlowPhase = "idle" | "renaming" | "indexing";

export interface LibraryController {
  info: LibraryInfo | null;
  remembered: string | null;
  folders: FolderNode[];
  items: GridItem[];
  progress: Progress | null;
  /** Failures from the current index run, per file. */
  failures: IndexFailure[];
  loading: boolean;
  error: string | null;
  scope: Scope;
  refreshToken: number;
  /** Set once a chosen folder turns out to need the import ceremony — the
   *  Review screen's entire content. `null` the rest of the time. */
  pendingReview: PendingReview | null;
  flowPhase: FlowPhase;
  /** Progress events during the "renaming" phase only. */
  renameProgress: ImportProgress | null;
  /** Set only if the post-import verification found a problem — surfaced
   *  once, silently absent otherwise, per docs/DESIGN.md#first-import. */
  verifyIssue: VerifyReport | null;
  /** Pick a library folder — the first one, or a different one later. */
  choose: () => void;
  open: (path: string) => void;
  /** Review → Import. Renames everything staged, then opens the library. */
  confirmImport: (confirmedBackup: boolean) => void;
  /** Review → Cancel. Back to the picker; nothing on disk has changed. */
  cancelImport: () => void;
  dismissVerifyIssue: () => void;
  retry: () => void;
  setScope: (scope: Scope) => void;
  /** Re-read the current view. Every mutation ends in one of these, so the
   *  grid, the tree and the counts all agree again. */
  reload: () => void;
  dismissError: () => void;
  /** Re-fetch just the folder tree — for after a folder-header edit (title,
   *  status, favorite) that the sidebar needs to reflect. */
  refreshFolders: () => void;
}

/**
 * While a library is being indexed the grid keeps filling in. Reloading the
 * whole item list is cheap early on and expensive later, so it is reloaded on
 * a timer only while the library is still small, and once more when the queue
 * finally goes quiet.
 */
const RELOAD_INTERVAL_MS = 4000;
const RELOAD_WHILE_UNDER = 20000;
const VERIFY_SAMPLE = 50;
/** How often the Progress screen re-asks the queue whether it has settled. */
const SETTLE_POLL_MS = 400;

export function useLibrary(): LibraryController {
  const [info, setInfo] = useState<LibraryInfo | null>(null);
  const [remembered, setRemembered] = useState<string | null>(null);
  const [folders, setFolders] = useState<FolderNode[]>([]);
  const [items, setItems] = useState<GridItem[]>([]);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScopeState] = useState<Scope>(EVERYTHING);
  const [refreshToken, setRefreshToken] = useState(0);
  const [failures, setFailures] = useState<IndexFailure[]>([]);
  const [pendingReview, setPendingReview] = useState<PendingReview | null>(null);
  const [flowPhase, setFlowPhase] = useState<FlowPhase>("idle");
  const [renameProgress, setRenameProgress] = useState<ImportProgress | null>(null);
  const [verifyIssue, setVerifyIssue] = useState<VerifyReport | null>(null);

  const scopeRef = useRef(scope);
  scopeRef.current = scope;
  const itemCount = useRef(0);
  itemCount.current = items.length;
  const lastReload = useRef(0);
  /** Library-wide item count as of the last reload — the grid holds a scoped
   *  subset of it, so this is what "there is new work to show" compares. */
  const loadedAt = useRef(-1);
  /** Failure count the list was last fetched for, so it is only re-fetched
   *  when the number actually moves. */
  const loadedFailures = useRef(-1);

  const syncFailures = useCallback(async (count: number) => {
    if (count === loadedFailures.current) return;
    loadedFailures.current = count;
    if (count === 0) {
      setFailures([]);
      return;
    }
    try {
      setFailures(await ipc.indexFailures());
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, []);

  const load = useCallback(async (next: Scope) => {
    try {
      const asked = query(next);
      const [rows, tree] = await Promise.all([
        ipc.listItems(asked.folder, asked.recursive),
        ipc.folderTree(),
      ]);
      // Favourites is filtered here rather than in SQL on purpose: the grid
      // already takes the whole manifest in one call (see `list_items`), so
      // this adds no query path for PLAN.md decision 20 to have to verify at
      // 100k, and no index to maintain.
      setItems(next.kind === "favourites" ? rows.filter((row) => row.favorite) : rows);
      setFolders(tree);
      lastReload.current = Date.now();
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, []);

  /** The actual open: assumes the library has already been imported (or
   *  never needed to be) — no Review, no Progress, just load and, for a
   *  library with nothing indexed yet, start the walk. */
  const openReal = useCallback(
    async (path: string) => {
      setItems([]);
      setFolders([]);
      setFailures([]);
      setProgress(null);
      loadedAt.current = -1;
      loadedFailures.current = -1;

      const opened = await ipc.openLibrary(path);
      setInfo(opened);
      setRemembered(opened.root);
      const next = EVERYTHING;
      setScopeState(next);
      await load(next);
      const started = await ipc.indexProgress();
      setProgress(started);
      loadedAt.current = started.items;
      await syncFailures(started.failed);
      // A library that has never been indexed is the reason the user just
      // pointed at it.
      if (opened.itemCount === 0) await ipc.startIndex();
      return opened;
    },
    [load, syncFailures],
  );

  const open = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      setPendingReview(null);
      try {
        const report = await ipc.prepareImport(path);
        if (report.alreadyImported) {
          await openReal(path);
        } else {
          setPendingReview({ path, report });
        }
      } catch (err) {
        setError(ipc.errorMessage(err));
      } finally {
        setLoading(false);
      }
    },
    [openReal],
  );

  const choose = useCallback(async () => {
    try {
      const picked = await ipc.pickLibraryFolder();
      if (picked) await open(picked);
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, [open]);

  const confirmImport = useCallback(
    async (confirmedBackup: boolean) => {
      if (!pendingReview) return;
      const { path } = pendingReview;

      setError(null);
      setFlowPhase("renaming");
      setRenameProgress(null);
      const unlisten = await ipc.onImportProgress(setRenameProgress);

      let executed: FsExecuteReport | null = null;
      try {
        executed = await ipc.executePreparedImport(confirmedBackup);
      } catch (err) {
        setError(ipc.errorMessage(err));
        setFlowPhase("idle");
        unlisten();
        return;
      }
      unlisten();

      if (executed.errors.length > 0) {
        // Not fatal — anything missed here is picked up automatically once
        // indexing renames it on the way in, per docs/DESIGN.md#first-import,
        // "After the first import". Nothing to surface but pressing on.
      }

      setPendingReview(null);
      try {
        await openReal(path);
        setFlowPhase("indexing");
      } catch (err) {
        setError(ipc.errorMessage(err));
        setFlowPhase("idle");
      }
    },
    [pendingReview, openReal],
  );

  const cancelImport = useCallback(async () => {
    setPendingReview(null);
    try {
      await ipc.cancelPreparedImport();
    } catch {
      // Best-effort — there is nothing on disk to undo either way.
    }
  }, []);

  const retry = useCallback(async () => {
    try {
      setFailures([]);
      loadedFailures.current = 0;
      await ipc.retryFailedJobs();
      setProgress(await ipc.indexProgress());
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, []);

  const setScope = useCallback(
    (next: Scope) => {
      setScopeState(next);
      void load(next);
    },
    [load],
  );

  /** Re-read the current view — what every mutation calls once it lands. */
  const reload = useCallback(() => {
    void load(scopeRef.current);
  }, [load]);

  const refreshFolders = useCallback(() => {
    (async () => {
      try {
        setFolders(await ipc.folderTree());
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  }, []);

  // Reopen whatever was open last time, without a picker.
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const status = await ipc.currentLibrary();
        if (cancelled) return;
        setRemembered(status.remembered);
        if (status.info) {
          setInfo(status.info);
          await load(EVERYTHING);
          const current = await ipc.indexProgress();
          setProgress(current);
          loadedAt.current = current.items;
          await syncFailures(current.failed);
        } else if (status.remembered) {
          await open(status.remembered);
          return;
        }
      } catch (err) {
        if (!cancelled) setError(ipc.errorMessage(err));
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const unlisten = ipc.onProgress((next) => {
      setProgress(next);
      // Thumbnails appear behind their items; nudge the pool to re-request
      // the ones that were not on disk when they were first drawn.
      setRefreshToken((token) => token + 1);

      const stale = next.items !== loadedAt.current;
      const settled = next.phase === "idle" && stale;
      const due =
        next.phase !== "idle" &&
        stale &&
        itemCount.current < RELOAD_WHILE_UNDER &&
        Date.now() - lastReload.current > RELOAD_INTERVAL_MS;

      if (settled || due) {
        loadedAt.current = next.items;
        void load(scopeRef.current);
      }
      void syncFailures(next.failed);
    });
    return () => {
      void unlisten.then((off) => off());
    };
  }, [load, syncFailures]);

  // The "indexing" leg of the Progress screen: wait for the walk+hash+thumb
  // queue to genuinely settle, run verification silently, then hand off to
  // the gallery. Verification surfaces only on failure — DESIGN.md#first-import.
  //
  // This **asks** the queue rather than waiting to be told. Progress events are
  // emitted only when the numbers change, so a library small enough to finish
  // indexing before this screen appears emits its last event with nobody
  // listening and then goes quiet forever — and the screen waits for an event
  // that is never coming. Twenty-three files is small enough. Polling here is
  // bounded and cheap: it runs only while this screen is up, and it stops the
  // moment the queue is empty.
  //
  // An `idle` reading is trusted outright, with no "have I seen it busy yet?"
  // guard, because `openReal` has already awaited `startIndex` by the time the
  // phase becomes "indexing" — the job is queued before anyone looks. The
  // ambiguity that guard existed for cannot arise here.
  useEffect(() => {
    if (flowPhase !== "indexing") return;

    let cancelled = false;
    let timer: number | undefined;

    const settle = async () => {
      let current: Progress;
      try {
        current = await ipc.indexProgress();
      } catch {
        // A failed read is not a reason to strand the user on this screen.
        if (!cancelled) setFlowPhase("idle");
        return;
      }
      if (cancelled) return;
      setProgress(current);

      if (current.phase !== "idle") {
        timer = window.setTimeout(() => void settle(), SETTLE_POLL_MS);
        return;
      }

      try {
        const verify = await ipc.verifyImport(VERIFY_SAMPLE);
        const clean =
          verify.mismatches.length === 0 &&
          verify.missing.length === 0 &&
          verify.countRenamed === verify.countTotal;
        if (!cancelled && !clean) setVerifyIssue(verify);
      } catch {
        // Best-effort — a failed check here should not block reaching the
        // gallery, only a failed verification should.
      } finally {
        if (!cancelled) setFlowPhase("idle");
      }
    };

    void settle();
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [flowPhase]);

  return {
    info,
    remembered,
    folders,
    items,
    progress,
    failures,
    loading,
    error,
    scope,
    refreshToken,
    pendingReview,
    flowPhase,
    renameProgress,
    verifyIssue,
    choose,
    open,
    confirmImport,
    cancelImport,
    dismissVerifyIssue: () => setVerifyIssue(null),
    retry,
    setScope,
    reload,
    dismissError: () => setError(null),
    refreshFolders,
  };
}
