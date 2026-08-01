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

export interface Scope {
  /** Folder rel_path, or null for the whole library. */
  folder: string | null;
  /** Folder views are recursive by default — PLAN.md decision 10. */
  recursive: boolean;
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
  reindex: () => void;
  retry: () => void;
  setScope: (scope: Scope) => void;
  dismissError: () => void;
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

export function useLibrary(): LibraryController {
  const [info, setInfo] = useState<LibraryInfo | null>(null);
  const [remembered, setRemembered] = useState<string | null>(null);
  const [folders, setFolders] = useState<FolderNode[]>([]);
  const [items, setItems] = useState<GridItem[]>([]);
  const [progress, setProgress] = useState<Progress | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [scope, setScopeState] = useState<Scope>({
    folder: null,
    recursive: true,
  });
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
  /** A brand-new library's very first progress read is "idle" too — nothing
   *  has been queued yet, before the walk even starts — indistinguishable
   *  from "idle because indexing finished" by phase alone. Only trusted once
   *  a busy phase has actually been observed during this flow. */
  const everBusyDuringFlow = useRef(false);

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
      const [rows, tree] = await Promise.all([
        ipc.listItems(next.folder, next.recursive),
        ipc.folderTree(),
      ]);
      setItems(rows);
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
      const next = { folder: null, recursive: true };
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
      everBusyDuringFlow.current = false;
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

  const reindex = useCallback(async () => {
    try {
      // The run clears the previous run's failures as it starts; clearing
      // them here too keeps the UI from showing a stale list in the gap
      // before the first progress tick.
      setFailures([]);
      loadedFailures.current = 0;
      await ipc.startIndex();
      setProgress(await ipc.indexProgress());
    } catch (err) {
      setError(ipc.errorMessage(err));
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
          await load({ folder: null, recursive: true });
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
  useEffect(() => {
    if (flowPhase !== "indexing" || !progress) return;

    if (progress.phase !== "idle") {
      everBusyDuringFlow.current = true;
      return;
    }
    const trustworthy = (info?.itemCount ?? 0) > 0 || everBusyDuringFlow.current;
    if (!trustworthy) return;

    let cancelled = false;
    (async () => {
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
    })();
    return () => {
      cancelled = true;
    };
  }, [flowPhase, progress, info]);

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
    reindex,
    retry,
    setScope,
    dismissError: () => setError(null),
  };
}
