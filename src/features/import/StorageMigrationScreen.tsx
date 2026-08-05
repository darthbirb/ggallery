import { useState } from "react";

import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { formatBytes, formatCount } from "../../lib/format";
import type { StorageMigrationState } from "../../state/library";

interface StorageMigrationScreenProps {
  state: StorageMigrationState;
  onCancel: () => void;
  onConfirm: (confirmedBackup: boolean) => void;
}

/**
 * PLAN.md §M2.6 — a real pre-existing library that has not yet moved every
 * file to `files/<xx>/<uuid>.<ext>`. The single most dangerous operation the
 * app performs, so this reuses the M1.7 startup flow's shape (one Review, one
 * Progress, a backup checkbox) rather than inventing a new one — but stays
 * deliberately minimal next to that flow's polish: the safety this exercises
 * (`fs::shard`'s write-manifest-then-dry-run-then-execute-then-verify order)
 * is this milestone's point, not a new multi-screen wizard.
 */
export function StorageMigrationScreen({
  state,
  onCancel,
  onConfirm,
}: StorageMigrationScreenProps) {
  const [confirmed, setConfirmed] = useState(false);
  const busy = state.phase !== "review";

  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 bg-ground px-8 py-10 text-fg">
      <div className="flex w-full max-w-[560px] flex-col gap-5">
        <div className="flex flex-col gap-1">
          <h1 className="text-[22px] font-semibold tracking-tight">
            Move this library to the new storage layout
          </h1>
          <p className="truncate font-mono text-fg-dim" title={state.path}>
            {state.path}
          </p>
        </div>

        <p className="leading-relaxed text-fg-mid">
          Folders became data some time ago — every file now lives at a
          location derived from its own id, sharded across{" "}
          <span className="font-mono text-fg">files/</span>. This library
          predates that and needs a one-time move before it can be opened.
          Nothing is written to disk until you confirm below; a complete
          record of what will move is written first, and the old directories
          are left alone afterward.
        </p>

        {state.phase === "review" && (
          <ReviewBody state={state} confirmed={confirmed} onConfirmedChange={setConfirmed} />
        )}
        {state.phase === "executing" && (
          <ProgressBody label="Moving files — do not close the library or unplug anything." total={state.progress?.total} done={state.progress?.done} errors={state.progress?.errors} />
        )}
        {state.phase === "verifying" && (
          <p className="text-fg-mid">Verifying every file landed where it should…</p>
        )}

        {state.error && <p className="text-danger">{state.error}</p>}

        <div className="flex items-center gap-2">
          <Button size="lg" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          {state.phase === "review" && (
            <Button
              size="lg"
              variant="accent"
              className="ml-auto"
              onClick={() => onConfirm(confirmed)}
              disabled={!confirmed || !state.review}
            >
              Migrate
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}

function ReviewBody({
  state,
  confirmed,
  onConfirmedChange,
}: {
  state: StorageMigrationState;
  confirmed: boolean;
  onConfirmedChange: (value: boolean) => void;
}) {
  const review = state.review;
  if (!review) {
    return <p className="text-fg-mid">Scanning the library…</p>;
  }
  const { dryRun, foldersMerged } = review;

  return (
    <div className="flex flex-col gap-3">
      <p>
        <span className="font-semibold text-fg">{formatCount(dryRun.toMove)}</span> of{" "}
        {formatCount(dryRun.totalItems)} file{dryRun.totalItems === 1 ? "" : "s"} (
        {formatBytes(dryRun.totalBytes)}) will move to their new location.
        {dryRun.alreadyDone > 0 && (
          <> {formatCount(dryRun.alreadyDone)} are already there — a previous run got that far.</>
        )}
      </p>

      {foldersMerged.length > 0 && (
        <p className="text-fg-mid">
          Also resolved {formatCount(foldersMerged.length)} sibling folder
          {foldersMerged.length === 1 ? "" : "s"} left over from before titles were folded to
          lowercase:{" "}
          {foldersMerged.map((merge) => merge.originals.join(" + ") + " → " + merge.folded).join(", ")}
          .
        </p>
      )}

      {dryRun.unreadable > 0 && (
        <p className="text-danger">
          {formatCount(dryRun.unreadable)} file{dryRun.unreadable === 1 ? "" : "s"} could not be
          found and will be left as they are.
        </p>
      )}

      {dryRun.collisions.length > 0 && (
        <p className="text-danger">
          {formatCount(dryRun.collisions.length)} item
          {dryRun.collisions.length === 1 ? "" : "s"} have something already sitting at their new
          location — worth a look before continuing.
        </p>
      )}

      <Label
        htmlFor="storage-migration-backup-confirmed"
        className="items-start gap-2.5 rounded-[5px] border border-line bg-raised px-3 py-2.5 text-[14px] text-fg"
      >
        <Checkbox
          id="storage-migration-backup-confirmed"
          checked={confirmed}
          onCheckedChange={(checked) => onConfirmedChange(checked === true)}
          className="mt-0.5"
        />
        <span>I have a backup of this library somewhere else.</span>
      </Label>
    </div>
  );
}

function ProgressBody({
  label,
  total,
  done,
  errors,
}: {
  label: string;
  total: number | undefined;
  done: number | undefined;
  errors: number | undefined;
}) {
  const t = total ?? 0;
  const d = done ?? 0;
  const pct = t > 0 ? Math.min(100, Math.round((d / t) * 100)) : 0;

  return (
    <div className="flex flex-col gap-2">
      <p className="text-fg-mid">{label}</p>
      <div className="h-2 overflow-hidden rounded-full bg-raised">
        <div className="h-full bg-accent transition-[width]" style={{ width: `${pct}%` }} />
      </div>
      <p className="font-mono tabular-nums text-fg-dim">
        {formatCount(d)} / {formatCount(t)}
        {errors ? <span className="text-danger"> · {formatCount(errors)} errors</span> : null}
      </p>
    </div>
  );
}
