import { formatCount } from "../../lib/format";
import type { ImportProgress, Progress } from "../../lib/types";
import type { FlowPhase } from "../../state/library";

interface ProgressScreenProps {
  phase: Exclude<FlowPhase, "idle">;
  renameProgress: ImportProgress | null;
  indexProgress: Progress | null;
}

/**
 * Rename, then index, then thumbnails, as one continuous readout — per
 * docs/DESIGN.md#first-import. Verification runs after this, silently; it
 * has no screen of its own, and only ever surfaces if it fails.
 */
export function ProgressScreen({
  phase,
  renameProgress,
  indexProgress,
}: ProgressScreenProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 bg-ground px-8 py-10 text-fg">
      <div className="flex w-full max-w-[480px] flex-col gap-4">
        <h1 className="text-[22px] font-semibold tracking-tight">
          {phase === "renaming" ? "Renaming" : "Indexing"}
        </h1>

        {phase === "renaming" ? (
          <RenameBar progress={renameProgress} />
        ) : (
          <IndexBar progress={indexProgress} />
        )}

        <p className="text-fg-dim">
          {phase === "renaming"
            ? "Do not close the library or unplug anything."
            : "Reading files and generating thumbnails — the gallery opens as soon as this settles."}
        </p>
      </div>
    </div>
  );
}

function RenameBar({ progress }: { progress: ImportProgress | null }) {
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;

  return (
    <div className="flex flex-col gap-2">
      <div className="h-2 overflow-hidden rounded-full bg-raised">
        <div
          className="h-full bg-accent transition-[width]"
          style={{ width: `${pct}%` }}
        />
      </div>
      <p className="font-mono tabular-nums text-fg-dim">
        {formatCount(done)} / {formatCount(total)}
        {progress && progress.errors > 0 && (
          <span className="text-danger"> · {formatCount(progress.errors)} errors</span>
        )}
      </p>
    </div>
  );
}

function IndexBar({ progress }: { progress: Progress | null }) {
  const remaining = (progress?.pending ?? 0) + (progress?.running ?? 0);
  return (
    <div className="flex flex-col gap-1 font-mono tabular-nums text-fg-dim">
      <p>
        {formatCount(progress?.itemsChecked ?? 0)} checked ·{" "}
        {formatCount(progress?.queued ?? 0)} queued from inbox
      </p>
      <p>
        {formatCount(progress?.items ?? 0)} indexed
        {remaining > 0 && <> · {formatCount(remaining)} remaining</>}
        {progress && progress.failed > 0 && (
          <span className="text-danger"> · {formatCount(progress.failed)} failed</span>
        )}
      </p>
    </div>
  );
}
