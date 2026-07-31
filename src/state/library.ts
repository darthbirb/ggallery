import { useCallback, useEffect, useRef, useState } from "react";

import * as ipc from "../lib/ipc";
import type {
  FolderNode,
  GridItem,
  IndexFailure,
  LibraryInfo,
  Progress,
} from "../lib/types";

export interface Scope {
  /** Folder rel_path, or null for the whole library. */
  folder: string | null;
  /** Folder views are recursive by default — PLAN.md decision 10. */
  recursive: boolean;
}

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
  /** Pick a library folder — the first one, or a different one later. */
  choose: () => void;
  open: (path: string) => void;
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

  const open = useCallback(
    async (path: string) => {
      setLoading(true);
      setError(null);
      // Nothing from the previous library survives the switch — not its
      // items, not its folder tree, not its failures.
      setItems([]);
      setFolders([]);
      setFailures([]);
      setProgress(null);
      loadedAt.current = -1;
      loadedFailures.current = -1;

      try {
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
      } catch (err) {
        setError(ipc.errorMessage(err));
      } finally {
        setLoading(false);
      }
    },
    [load, syncFailures],
  );

  const choose = useCallback(async () => {
    try {
      const picked = await ipc.pickLibraryFolder();
      if (picked) await open(picked);
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  }, [open]);

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
  }, [load, open, syncFailures]);

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
    choose,
    open,
    reindex,
    retry,
    setScope,
    dismissError: () => setError(null),
  };
}
