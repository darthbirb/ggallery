import { Button } from "../../components/ui/button";
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
    <span className="flex items-center gap-2 font-mono tabular-nums text-fg-dim">
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
        <Button
          size="sm"
          variant="danger"
          onClick={onToggleFailures}
          aria-expanded={showingFailures}
          className="font-mono"
        >
          {formatCount(failureCount)} failed
        </Button>
      )}
    </span>
  );
}
