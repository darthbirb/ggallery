import { useMemo } from "react";

import { formatBytes, formatCount } from "../../lib/format";
import type { IndexFailure } from "../../lib/types";

interface FailureListProps {
  failures: IndexFailure[];
  onRetry: () => void;
  onClose: () => void;
}

/** Rows rendered before the list stops and says how many more there are. */
const MAX_ROWS = 300;

/**
 * What failed, which file, and what the decoder said.
 *
 * The count on its own is what made the original defect hard to diagnose: six
 * broken files across three index runs read as "18 failed", with nothing
 * naming a single one of them.
 */
export function FailureList({ failures, onRetry, onClose }: FailureListProps) {
  // Grouped by message, because a real library fails the same way many times
  // over, and the shape of the problem is the useful part.
  const byError = useMemo(() => {
    const counts = new Map<string, number>();
    for (const failure of failures) {
      counts.set(failure.error, (counts.get(failure.error) ?? 0) + 1);
    }
    return [...counts.entries()].sort((a, b) => b[1] - a[1]);
  }, [failures]);

  return (
    <section className="flex max-h-[45%] min-h-0 flex-col border-b border-line bg-panel">
      <header className="flex items-center gap-3 px-3 py-2">
        <span className="font-semibold text-danger">
          {formatCount(failures.length)}{" "}
          {failures.length === 1 ? "file" : "files"} failed to index
        </span>
        <span className="font-mono text-[11px] text-fg-dim">
          everything else indexed normally
        </span>

        <button
          type="button"
          onClick={onRetry}
          className="ml-auto rounded-[3px] border border-line px-2 py-0.5 text-fg-mid hover:bg-hover hover:text-fg"
        >
          Retry these
        </button>
        <button
          type="button"
          onClick={onClose}
          className="rounded-[3px] border border-transparent px-2 py-0.5 text-fg-dim hover:bg-hover hover:text-fg"
        >
          Close
        </button>
      </header>

      {byError.length > 1 && (
        <ul className="flex flex-wrap gap-x-4 gap-y-1 px-3 pb-2 font-mono text-[11px] text-fg-dim">
          {byError.map(([error, count]) => (
            <li key={error}>
              <span className="text-fg-mid">{count}×</span> {error}
            </li>
          ))}
        </ul>
      )}

      <div className="min-h-0 overflow-y-auto border-t border-line-soft">
        <table className="w-full border-collapse text-left">
          <tbody>
            {failures.slice(0, MAX_ROWS).map((failure) => (
              <tr key={failure.jobId} className="border-b border-line-soft/60">
                <td className="max-w-0 truncate px-3 py-1 font-mono text-[11px] text-fg">
                  <span className="text-fg-dim">
                    {failure.folder ? `${failure.folder}/` : ""}
                  </span>
                  {failure.name}
                </td>
                <td className="whitespace-nowrap px-2 py-1 font-mono text-[10px] text-fg-dim">
                  {failure.stage}
                </td>
                <td className="whitespace-nowrap px-2 py-1 text-right font-mono text-[10px] tabular-nums text-fg-dim">
                  {failure.sizeBytes === null ? "" : formatBytes(failure.sizeBytes)}
                </td>
                <td className="px-3 py-1 text-[12px] text-danger">{failure.error}</td>
              </tr>
            ))}
          </tbody>
        </table>

        {failures.length > MAX_ROWS && (
          <p className="px-3 py-2 font-mono text-[11px] text-fg-dim">
            …and {formatCount(failures.length - MAX_ROWS)} more.
          </p>
        )}
      </div>
    </section>
  );
}
