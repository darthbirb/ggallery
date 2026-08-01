import { useEffect, useState } from "react";

import { formatBytes, formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type {
  DryRunReport,
  ExecuteReport,
  ImportProgress,
  ScanReport,
  VerifyReport,
} from "../../lib/types";

type Step = "scan" | "dryRun" | "backup" | "execute" | "done";

const DRY_RUN_SAMPLE = 20;
const VERIFY_SAMPLE = 50;

interface ImportWizardProps {
  onClose: () => void;
  /** "First import" when offered automatically while opening a library that
   *  has never been imported; "Normalise filenames" for the repair case,
   *  triggered manually from Settings. Same flow either way — DESIGN.md's
   *  "same gate" — just different framing for why it's running. */
  title?: string;
  /** False for the automatic first-import offer: it is a step in opening the
   *  library, not a dismissable suggestion, so there is no way out except
   *  through the backup gate. The manual "Normalise filenames" entry point
   *  is something the user chose to open, so it stays closable throughout. */
  dismissable?: boolean;
}

/**
 * The scan / dry run / backup acknowledgement / batched execution / verify
 * flow, per docs/DESIGN.md#first-import. Used two ways: offered automatically
 * when opening a library that has never been imported, and manually via
 * Settings → Normalise filenames for the repair case.
 *
 * Scoped to the rename alone. Parsing folder names into archetype fields is a
 * separate M2 step that needs archetypes to exist first.
 */
export function ImportWizard({
  onClose,
  title = "First import — rename files to UUIDs",
  dismissable = true,
}: ImportWizardProps) {
  const [step, setStep] = useState<Step>("scan");
  const [scan, setScan] = useState<ScanReport | null>(null);
  const [dryRun, setDryRun] = useState<DryRunReport | null>(null);
  const [confirmedBackup, setConfirmedBackup] = useState(false);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [executeReport, setExecuteReport] = useState<ExecuteReport | null>(null);
  const [verifyReport, setVerifyReport] = useState<VerifyReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Renaming must not be interrupted by closing the wizard mid-batch — the
  // operation itself is crash-safe, but there is no reason to invite it. The
  // automatic first-import offer additionally has no close button at all,
  // at any step — see the `dismissable` prop.
  const closable = dismissable && step !== "execute";

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setBusy(true);
      try {
        const report = await ipc.scanImport();
        if (cancelled) return;
        setScan(report);
        if (report.toRename === 0) setStep("done");
      } catch (err) {
        if (!cancelled) setError(ipc.errorMessage(err));
      } finally {
        if (!cancelled) setBusy(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const goDryRun = async () => {
    setBusy(true);
    setError(null);
    try {
      setDryRun(await ipc.dryRunImport(DRY_RUN_SAMPLE));
      setStep("dryRun");
    } catch (err) {
      setError(ipc.errorMessage(err));
    } finally {
      setBusy(false);
    }
  };

  const goExecute = async () => {
    setStep("execute");
    setBusy(true);
    setError(null);
    setProgress(null);
    const unlisten = await ipc.onImportProgress(setProgress);
    try {
      const executed = await ipc.executeImport(confirmedBackup);
      setExecuteReport(executed);
      setVerifyReport(await ipc.verifyImport(VERIFY_SAMPLE));
      setStep("done");
    } catch (err) {
      setError(ipc.errorMessage(err));
    } finally {
      setBusy(false);
      unlisten();
    }
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[82vh] w-[560px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">{title}</span>
          {dismissable && (
            <button
              type="button"
              onClick={onClose}
              disabled={!closable}
              title={closable ? "Close" : "Renaming is in progress"}
              className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg disabled:opacity-30"
            >
              ✕
            </button>
          )}
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4 text-[13px] text-fg-mid">
          {error && (
            <p className="mb-3 rounded-[3px] border border-danger/40 bg-raised px-3 py-2 text-danger">
              {error}
            </p>
          )}

          {step === "scan" && <ScanStep scan={scan} busy={busy && !scan} />}
          {step === "dryRun" && dryRun && <DryRunStep report={dryRun} />}
          {step === "backup" && (
            <BackupStep
              toRename={scan?.toRename ?? 0}
              confirmed={confirmedBackup}
              onChange={setConfirmedBackup}
            />
          )}
          {step === "execute" && <ExecuteStep progress={progress} />}
          {step === "done" && (
            <DoneStep
              scan={scan}
              executeReport={executeReport}
              verifyReport={verifyReport}
            />
          )}
        </div>

        <footer className="flex items-center gap-2 border-t border-line px-4 py-3">
          {step === "scan" && (
            <button
              type="button"
              onClick={goDryRun}
              disabled={busy || !scan}
              className="ml-auto rounded-[3px] border border-accent-d bg-raised px-3 py-1.5 text-accent hover:bg-hover disabled:opacity-40"
            >
              Continue
            </button>
          )}

          {step === "dryRun" && (
            <>
              <button
                type="button"
                onClick={() => setStep("scan")}
                className="rounded-[3px] border border-line px-3 py-1.5 text-fg-mid hover:bg-hover hover:text-fg"
              >
                Back
              </button>
              <button
                type="button"
                onClick={() => setStep("backup")}
                className="ml-auto rounded-[3px] border border-accent-d bg-raised px-3 py-1.5 text-accent hover:bg-hover"
              >
                Continue
              </button>
            </>
          )}

          {step === "backup" && (
            <>
              <button
                type="button"
                onClick={() => setStep("dryRun")}
                className="rounded-[3px] border border-line px-3 py-1.5 text-fg-mid hover:bg-hover hover:text-fg"
              >
                Back
              </button>
              <button
                type="button"
                onClick={goExecute}
                disabled={!confirmedBackup || busy}
                className="ml-auto rounded-[3px] border border-danger bg-raised px-3 py-1.5 text-danger hover:bg-hover disabled:opacity-40"
              >
                Rename now
              </button>
            </>
          )}

          {step === "done" && (
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

function ScanStep({ scan, busy }: { scan: ScanReport | null; busy: boolean }) {
  if (busy || !scan) {
    return <p>Scanning the library…</p>;
  }

  if (scan.toRename === 0) {
    return (
      <p>
        Every file already carries its UUID name — there is nothing to rename.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      <p>What M1's index already knows about the library:</p>

      <table className="w-full border-collapse text-left font-mono text-[12px]">
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
          <tr className="border-b border-line-soft/60">
            <td className="py-1 pr-3 text-fg-dim">folders</td>
            <td className="py-1 tabular-nums text-fg" colSpan={2}>
              {formatCount(scan.folderCount)}
            </td>
          </tr>
        </tbody>
      </table>

      {scan.unreadable > 0 && (
        <p className="text-danger">
          {formatCount(scan.unreadable)} file
          {scan.unreadable === 1 ? "" : "s"} could not be read at all and will
          be left as they are.
        </p>
      )}

      <p>
        <span className="font-semibold text-fg">
          {formatCount(scan.toRename)}
        </span>{" "}
        file{scan.toRename === 1 ? "" : "s"} will be renamed to a UUID.
        {scan.alreadyRenamed > 0 && (
          <> {formatCount(scan.alreadyRenamed)} already carry one.</>
        )}
      </p>
    </div>
  );
}

function DryRunStep({ report }: { report: DryRunReport }) {
  return (
    <div className="flex flex-col gap-3">
      <p>
        <span className="font-semibold text-fg">
          {formatCount(report.toRename)}
        </span>{" "}
        file{report.toRename === 1 ? "" : "s"} will be renamed. Nothing has
        been written yet — this is exactly what will happen:
      </p>

      <div className="max-h-[280px] overflow-y-auto rounded-[3px] border border-line-soft">
        <table className="w-full border-collapse text-left font-mono text-[11px]">
          <tbody>
            {report.sample.map((row, i) => (
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

      {report.toRename > report.sample.length && (
        <p className="text-fg-dim">
          …and {formatCount(report.toRename - report.sample.length)} more,
          shown the same way.
        </p>
      )}
    </div>
  );
}

function BackupStep({
  toRename,
  confirmed,
  onChange,
}: {
  toRename: number;
  confirmed: boolean;
  onChange: (value: boolean) => void;
}) {
  return (
    <div className="flex flex-col gap-3">
      <p>
        This renames {formatCount(toRename)} file
        {toRename === 1 ? "" : "s"} on disk. It is the most destructive thing
        this app does, and it cannot be undone from inside the app once it
        starts — only by running the separate reversal tool against{" "}
        <span className="font-mono text-fg">library.jsonl</span> afterward.
      </p>
      <p>Do not proceed without a backup that lives somewhere else.</p>

      <label className="flex items-start gap-2 rounded-[3px] border border-line-soft bg-raised px-3 py-2">
        <input
          type="checkbox"
          checked={confirmed}
          onChange={(event) => onChange(event.target.checked)}
          className="mt-0.5 accent-accent"
        />
        <span>I have a backup of this library somewhere else.</span>
      </label>
    </div>
  );
}

function ExecuteStep({ progress }: { progress: ImportProgress | null }) {
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

      <p className="font-mono text-[12px] tabular-nums text-fg-dim">
        {formatCount(done)} / {formatCount(total)}
        {progress && progress.errors > 0 && (
          <span className="text-danger"> · {formatCount(progress.errors)} errors</span>
        )}
      </p>

      <p className="text-fg-dim">
        Every batch is written to{" "}
        <span className="font-mono text-fg">library.jsonl</span> before its
        files are touched, so this can always be resumed if it is
        interrupted.
      </p>
    </div>
  );
}

function DoneStep({
  scan,
  executeReport,
  verifyReport,
}: {
  scan: ScanReport | null;
  executeReport: ExecuteReport | null;
  verifyReport: VerifyReport | null;
}) {
  if (!executeReport) {
    return (
      <p>
        Nothing needed renaming
        {scan ? ` — ${formatCount(scan.alreadyRenamed)} files already carried a UUID name.` : "."}
      </p>
    );
  }

  const countsMatch = verifyReport
    ? verifyReport.countRenamed === verifyReport.countTotal
    : false;
  const verifyClean =
    verifyReport &&
    verifyReport.mismatches.length === 0 &&
    verifyReport.missing.length === 0;

  return (
    <div className="flex flex-col gap-3">
      <p>
        <span className="font-semibold text-fg">
          {formatCount(executeReport.renamed)}
        </span>{" "}
        file{executeReport.renamed === 1 ? "" : "s"} renamed.
        {executeReport.alreadyDone > 0 && (
          <> {formatCount(executeReport.alreadyDone)} already had a UUID name.</>
        )}
      </p>

      {executeReport.errors.length > 0 && (
        <div className="rounded-[3px] border border-danger/40 bg-raised px-3 py-2">
          <p className="text-danger">
            {formatCount(executeReport.errors.length)} file
            {executeReport.errors.length === 1 ? "" : "s"} could not be
            renamed:
          </p>
          <ul className="mt-1 max-h-[120px] overflow-y-auto font-mono text-[11px] text-fg-dim">
            {executeReport.errors.map((err) => (
              <li key={err.itemId}>
                {err.folder ? `${err.folder}/` : ""}
                {err.name} — {err.error}
              </li>
            ))}
          </ul>
        </div>
      )}

      {verifyReport && (
        <div
          className={`rounded-[3px] border px-3 py-2 ${
            verifyClean && countsMatch
              ? "border-line-soft bg-raised"
              : "border-danger/40 bg-raised text-danger"
          }`}
        >
          <p className="font-semibold">
            {verifyClean && countsMatch ? "Verified." : "Verification found a problem."}
          </p>
          <p className="text-[12px]">
            Re-hashed {formatCount(verifyReport.sampleChecked)} random file
            {verifyReport.sampleChecked === 1 ? "" : "s"} — all matched their
            recorded content hash.{" "}
            {countsMatch
              ? `${formatCount(verifyReport.countRenamed)} of ${formatCount(verifyReport.countTotal)} items carry a UUID name.`
              : `Only ${formatCount(verifyReport.countRenamed)} of ${formatCount(verifyReport.countTotal)} items carry a UUID name.`}
          </p>
          {verifyReport.mismatches.length > 0 && (
            <p className="text-[12px]">
              {formatCount(verifyReport.mismatches.length)} sampled file
              {verifyReport.mismatches.length === 1 ? "" : "s"} did not match
              its recorded hash.
            </p>
          )}
          {verifyReport.missing.length > 0 && (
            <p className="text-[12px]">
              {formatCount(verifyReport.missing.length)} sampled file
              {verifyReport.missing.length === 1 ? "" : "s"} could not be
              found at its new path.
            </p>
          )}
        </div>
      )}
    </div>
  );
}
