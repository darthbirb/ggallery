import { formatCount } from "../../lib/format";
import type { Progress } from "../../lib/types";

interface IndexStatusProps {
  progress: Progress | null;
  /** Non-zero only while the current run has failures to show. */
  failureCount: number;
  showingFailures: boolean;
  onToggleFailures: () => void;
}

/** The topbar readout: what the queue is doing, and a way into the failures. */
export function IndexStatus({
  progress,
  failureCount,
  showingFailures,
  onToggleFailures,
}: IndexStatusProps) {
  if (!progress) return null;

  return (
    <span className="flex items-center gap-2 font-mono text-[11px] tabular-nums text-fg-dim">
      {progress.phase === "walking" && (
        <span className="text-fg-mid">
          {progress.rescanning
            ? "rescanning — the watcher lost sync, redoing a full scan"
            : "scanning"}{" "}
          · {formatCount(progress.folders)} folders ·{" "}
          {formatCount(progress.filesSeen)} files
        </span>
      )}

      {progress.phase === "working" && (
        <span className="text-fg-mid">
          indexing · {formatCount(progress.items)} items ·{" "}
          {formatCount(progress.pending + progress.running)} queued
        </span>
      )}

      {progress.phase === "idle" && <span>{formatCount(progress.items)} items</span>}

      {failureCount > 0 && (
        <button
          type="button"
          onClick={onToggleFailures}
          aria-expanded={showingFailures}
          className={`rounded-[3px] border px-2 py-0.5 ${
            showingFailures
              ? "border-danger bg-raised text-danger"
              : "border-transparent text-danger hover:bg-raised"
          }`}
        >
          {formatCount(failureCount)} failed
        </button>
      )}
    </span>
  );
}
