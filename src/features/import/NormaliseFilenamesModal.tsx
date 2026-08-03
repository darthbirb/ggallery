import { useEffect, useState } from "react";

import { Dialog } from "../../components/Dialog";
import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { formatBytes, formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type {
  DryRunReport,
  ExecuteReport,
  ImportProgress,
  ScanReport,
  VerifyReport,
} from "../../lib/types";
import { useToasts } from "../../state/toasts";

type Step = "loading" | "review" | "progress" | "done";

const SAMPLE = 5;
const VERIFY_SAMPLE = 50;

interface NormaliseFilenamesModalProps {
  onClose: () => void;
}

/**
 * Settings → Normalise filenames — the repair case: an already-open,
 * already-indexed library that has something odd-named in it. Same gate as
 * the startup flow (one Review, one Progress, verification runs silently)
 * but against the database this library already has, and as a modal rather
 * than a full-window screen, since there is a gallery to come back to. See
 * docs/DESIGN.md#first-import, "Repairing later".
 */
export function NormaliseFilenamesModal({
  onClose,
}: NormaliseFilenamesModalProps) {
  const toasts = useToasts();
  const [step, setStep] = useState<Step>("loading");
  const [scan, setScan] = useState<ScanReport | null>(null);
  const [dryRun, setDryRun] = useState<DryRunReport | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [executed, setExecuted] = useState<ExecuteReport | null>(null);
  const [verifyIssue, setVerifyIssue] = useState<VerifyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const scanned = await ipc.scanImport();
        if (cancelled) return;
        if (scanned.toRename === 0) {
          await ipc.markImported();
          if (cancelled) return;
          // Otherwise this closes silently and reads as the button having
          // done nothing at all — there is no review screen to say so when
          // there is nothing to review.
          toasts.push({ message: "Everything already has a UUID name." });
          onClose();
          return;
        }
        const preview = await ipc.dryRunImport(SAMPLE);
        if (cancelled) return;
        setScan(scanned);
        setDryRun(preview);
        setStep("review");
      } catch (err) {
        if (!cancelled) setError(ipc.errorMessage(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [onClose]);

  const closable = step !== "progress";

  const run = async () => {
    setStep("progress");
    setBusy(true);
    setError(null);
    const unlisten = await ipc.onImportProgress(setProgress);
    try {
      const report = await ipc.executeImport(confirmed);
      setExecuted(report);
      const verify = await ipc.verifyImport(VERIFY_SAMPLE);
      const clean =
        verify.mismatches.length === 0 &&
        verify.missing.length === 0 &&
        verify.countRenamed === verify.countTotal;
      if (!clean) setVerifyIssue(verify);
      setStep("done");
    } catch (err) {
      setError(ipc.errorMessage(err));
    } finally {
      setBusy(false);
      unlisten();
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      closable={closable}
      title="Normalise filenames"
      description="For when something outside the app has renamed a file back."
      width={580}
      footer={
        step === "review" ? (
          <Button
            variant="danger"
            disabled={!confirmed || busy}
            onClick={() => void run()}
          >
            Rename now
          </Button>
        ) : step === "done" ? (
          <Button variant="accent" onClick={onClose}>
            Close
          </Button>
        ) : undefined
      }
    >
      <div className="text-fg-mid">
        {error && (
          <p className="mb-3 rounded-[5px] border border-danger/40 bg-raised px-3 py-2 text-danger">
            {error}
          </p>
        )}

        {step === "loading" && <p>Scanning the library…</p>}

        {step === "review" && scan && dryRun && (
          <ReviewBody
            scan={scan}
            dryRun={dryRun}
            confirmed={confirmed}
            onConfirmedChange={setConfirmed}
          />
        )}

        {step === "progress" && <ProgressBody progress={progress} />}

        {step === "done" && executed && (
          <DoneBody executed={executed} verifyIssue={verifyIssue} />
        )}
      </div>
    </Dialog>
  );
}

function ReviewBody({
  scan,
  dryRun,
  confirmed,
  onConfirmedChange,
}: {
  scan: ScanReport;
  dryRun: DryRunReport;
  confirmed: boolean;
  onConfirmedChange: (value: boolean) => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <p>
        <span className="font-semibold text-fg">
          {formatCount(scan.toRename)}
        </span>{" "}
        file{scan.toRename === 1 ? "" : "s"} will be renamed to a UUID. The
        original name is kept and shown in each file's details.
        {scan.alreadyRenamed > 0 && (
          <> {formatCount(scan.alreadyRenamed)} already carry one.</>
        )}
      </p>

      <table className="w-full border-collapse text-left font-mono">
        <tbody>
          {scan.byKind.map((kind) => (
            <tr key={kind.kind} className="border-b border-line-soft/60">
              <td className="py-1 pr-3 text-fg-dim">{kind.kind}</td>
              <td className="py-1 pr-3 tabular-nums text-fg">
                {formatCount(kind.count)}
              </td>
              <td className="py-1 tabular-nums text-fg-dim">
                {formatBytes(kind.bytes)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>

      {scan.unreadable > 0 && (
        <p className="text-danger">
          {formatCount(scan.unreadable)} file
          {scan.unreadable === 1 ? "" : "s"} could not be read at all and
          will be left as they are.
        </p>
      )}

      <div className="max-h-[200px] overflow-y-auto rounded-[5px] border border-line-soft">
        <table className="w-full border-collapse text-left font-mono">
          <tbody>
            {dryRun.sample.map((row, i) => (
              <tr key={i} className="border-b border-line-soft/60">
                <td className="max-w-0 truncate px-2 py-1 text-fg-dim">
                  {row.folder ? `${row.folder}/` : ""}
                  {row.oldName}
                </td>
                <td className="px-1 py-1 text-fg-dim">→</td>
                <td className="max-w-0 truncate px-2 py-1 text-fg">
                  {row.newName}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {dryRun.toRename > dryRun.sample.length && (
        <p className="text-fg-dim">
          …and {formatCount(dryRun.toRename - dryRun.sample.length)} more,
          shown the same way.
        </p>
      )}

      <Label
        htmlFor="normalise-backup-confirmed"
        className="items-start gap-2.5 rounded-[5px] border border-line bg-raised px-3 py-2.5 text-[14px] text-fg"
      >
        <Checkbox
          id="normalise-backup-confirmed"
          checked={confirmed}
          onCheckedChange={(checked) => onConfirmedChange(checked === true)}
          className="mt-0.5"
        />
        <span>I have a backup of this library somewhere else.</span>
      </Label>
    </div>
  );
}

function ProgressBody({ progress }: { progress: ImportProgress | null }) {
  const total = progress?.total ?? 0;
  const done = progress?.done ?? 0;
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;

  return (
    <div className="flex flex-col gap-3">
      <p>Renaming — do not close the library or unplug anything.</p>

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

function DoneBody({
  executed,
  verifyIssue,
}: {
  executed: ExecuteReport;
  verifyIssue: VerifyReport | null;
}) {
  return (
    <div className="flex flex-col gap-3">
      <p>
        <span className="font-semibold text-fg">
          {formatCount(executed.renamed)}
        </span>{" "}
        file{executed.renamed === 1 ? "" : "s"} renamed.
      </p>

      {executed.errors.length > 0 && (
        <div className="rounded-[5px] border border-danger/40 bg-raised px-3 py-2">
          <p className="text-danger">
            {formatCount(executed.errors.length)} file
            {executed.errors.length === 1 ? "" : "s"} could not be renamed:
          </p>
          <ul className="mt-1 max-h-[120px] overflow-y-auto font-mono text-fg-dim">
            {executed.errors.map((err) => (
              <li key={err.itemId}>
                {err.folder ? `${err.folder}/` : ""}
                {err.name} — {err.error}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Verification runs silently and only appears here on failure — per
          docs/DESIGN.md#first-import. */}
      {verifyIssue && (
        <div className="rounded-[5px] border border-danger/40 bg-raised px-3 py-2 text-danger">
          <p className="font-semibold">Verification found a problem.</p>
          <p className="text-[13px]">
            {formatCount(verifyIssue.countRenamed)} of{" "}
            {formatCount(verifyIssue.countTotal)} items carry a UUID name.
            {verifyIssue.mismatches.length > 0 && (
              <>
                {" "}
                {formatCount(verifyIssue.mismatches.length)} sampled file
                {verifyIssue.mismatches.length === 1 ? "" : "s"} did not
                match its recorded hash.
              </>
            )}
            {verifyIssue.missing.length > 0 && (
              <>
                {" "}
                {formatCount(verifyIssue.missing.length)} sampled file
                {verifyIssue.missing.length === 1 ? "" : "s"} could not be
                found at its new path.
              </>
            )}
          </p>
        </div>
      )}
    </div>
  );
}
