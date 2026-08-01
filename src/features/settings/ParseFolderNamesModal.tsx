import { useEffect, useState } from "react";

import { formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { NameParseCandidate } from "../../lib/types";

interface ParseFolderNamesModalProps {
  onClose: () => void;
  /** The sidebar tree needs to reflect the new titles once applied. */
  onApplied: () => void;
}

/**
 * Settings → Parse folder names — the deferred M1.5 item, held over until
 * archetypes existed. Folders named `"Ana (@ana)"` are offered as
 * `title: Ana` with `instagram: @ana` on the Person archetype, in an
 * editable table, applied only on confirmation. Same one-screen-review shape
 * as `NormaliseFilenamesModal`.
 */
export function ParseFolderNamesModal({
  onClose,
  onApplied,
}: ParseFolderNamesModalProps) {
  const [candidates, setCandidates] = useState<NameParseCandidate[] | null>(null);
  const [excluded, setExcluded] = useState<Set<number>>(() => new Set());
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [done, setDone] = useState<number | null>(null);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const rows = await ipc.scanFolderNameParse();
        if (!cancelled) setCandidates(rows);
      } catch (err) {
        if (!cancelled) setError(ipc.errorMessage(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const update = (folderId: number, patch: Partial<NameParseCandidate>) => {
    setCandidates((rows) =>
      rows?.map((row) => (row.folderId === folderId ? { ...row, ...patch } : row)) ?? null,
    );
  };

  const toggle = (folderId: number) => {
    setExcluded((current) => {
      const next = new Set(current);
      if (next.has(folderId)) next.delete(folderId);
      else next.add(folderId);
      return next;
    });
  };

  const apply = async () => {
    if (!candidates) return;
    const rows = candidates.filter((c) => !excluded.has(c.folderId));
    setBusy(true);
    setError(null);
    try {
      await ipc.applyFolderNameParse(rows);
      setDone(rows.length);
      onApplied();
    } catch (err) {
      setError(ipc.errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const included = candidates?.filter((c) => !excluded.has(c.folderId)) ?? [];

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[82vh] w-[640px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Parse folder names</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 text-[13px] text-fg-mid">
          {error && (
            <p className="mb-3 rounded-[3px] border border-danger/40 bg-raised px-3 py-2 text-danger">
              {error}
            </p>
          )}

          {done !== null ? (
            <p>
              {formatCount(done)} folder{done === 1 ? "" : "s"} updated.
            </p>
          ) : candidates === null ? (
            <p>Scanning folder names…</p>
          ) : candidates.length === 0 ? (
            <p>
              No folder names look like <span className="font-mono">Name (@handle)</span> — nothing to do.
            </p>
          ) : (
            <div className="flex flex-col gap-3">
              <p>
                Folders named like <span className="font-mono">Ana (@ana)</span> can be
                split into a title and an Instagram handle on the Person
                archetype. Uncheck any row you don't want touched, or edit the
                values below.
              </p>

              <table className="w-full border-collapse text-left text-[12px]">
                <thead>
                  <tr className="text-fg-dim">
                    <th className="w-6 py-1"></th>
                    <th className="py-1 pr-2">folder</th>
                    <th className="py-1 pr-2">title</th>
                    <th className="py-1">instagram</th>
                  </tr>
                </thead>
                <tbody>
                  {candidates.map((row) => (
                    <tr key={row.folderId} className="border-t border-line-soft/60">
                      <td className="py-1">
                        <input
                          type="checkbox"
                          checked={!excluded.has(row.folderId)}
                          onChange={() => toggle(row.folderId)}
                          className="accent-accent"
                        />
                      </td>
                      <td className="max-w-0 truncate py-1 pr-2 font-mono text-fg-dim">
                        {row.relPath}
                      </td>
                      <td className="py-1 pr-2">
                        <input
                          value={row.proposedTitle}
                          onChange={(event) =>
                            update(row.folderId, { proposedTitle: event.target.value })
                          }
                          disabled={excluded.has(row.folderId)}
                          className="w-full rounded-[3px] border border-line-soft bg-ground px-1.5 py-0.5 text-fg disabled:opacity-40"
                        />
                      </td>
                      <td className="py-1">
                        <input
                          value={row.handle}
                          onChange={(event) => update(row.folderId, { handle: event.target.value })}
                          disabled={excluded.has(row.folderId)}
                          className="w-full rounded-[3px] border border-line-soft bg-ground px-1.5 py-0.5 text-fg disabled:opacity-40"
                        />
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        <footer className="flex items-center gap-2 border-t border-line px-4 py-3">
          {done === null && candidates && candidates.length > 0 && (
            <button
              type="button"
              onClick={apply}
              disabled={busy || included.length === 0}
              className="ml-auto rounded-[3px] border border-accent-d bg-raised px-3 py-1.5 text-accent hover:bg-hover disabled:opacity-40"
            >
              Apply to {formatCount(included.length)}
            </button>
          )}
          {(done !== null || (candidates && candidates.length === 0)) && (
            <button
              type="button"
              onClick={onClose}
              className="ml-auto rounded-[3px] border border-accent-d bg-raised px-3 py-1.5 text-accent hover:bg-hover"
            >
              Close
            </button>
          )}
        </footer>
      </div>
    </div>
  );
}
