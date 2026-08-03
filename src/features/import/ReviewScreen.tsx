import { Button } from "../../components/ui/button";
import { Checkbox } from "../../components/ui/checkbox";
import { Label } from "../../components/ui/label";
import { formatBytes, formatCount } from "../../lib/format";
import type { ReviewReport } from "../../lib/types";

interface ReviewScreenProps {
  path: string;
  report: ReviewReport;
  confirmed: boolean;
  onConfirmedChange: (value: boolean) => void;
  busy: boolean;
  error: string | null;
  onCancel: () => void;
  onImport: () => void;
}

/**
 * The startup flow's one substantive screen — counts, one sentence, a
 * five-row sample, one checkbox. Everything the old six-step wizard asked
 * across scan / dry run / backup collapses into this, per
 * docs/DESIGN.md#first-import.
 */
export function ReviewScreen({
  path,
  report,
  confirmed,
  onConfirmedChange,
  busy,
  error,
  onCancel,
  onImport,
}: ReviewScreenProps) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-6 bg-ground px-8 py-10 text-fg">
      <div className="flex w-full max-w-[560px] flex-col gap-5">
        <div className="flex flex-col gap-1">
          <h1 className="text-[22px] font-semibold tracking-tight">Review</h1>
          <p className="truncate font-mono text-fg-dim" title={path}>
            {path}
          </p>
        </div>

        <p className="leading-relaxed text-fg-mid">
          <span className="font-semibold text-fg">
            {formatCount(report.toRename)}
          </span>{" "}
          file{report.toRename === 1 ? "" : "s"} will be renamed to a UUID.
          The original name is kept and shown in each file's details.
          {report.alreadyRenamed > 0 && (
            <> {formatCount(report.alreadyRenamed)} already carry one.</>
          )}
        </p>

        <table className="w-full border-collapse text-left font-mono">
          <tbody>
            {report.byKind.map((kind) => (
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
            <tr>
              <td className="py-1 pr-3 text-fg-dim">folders</td>
              <td className="py-1 tabular-nums text-fg" colSpan={2}>
                {formatCount(report.folderCount)}
              </td>
            </tr>
          </tbody>
        </table>

        {report.unreadable > 0 && (
          <p className="text-danger">
            {formatCount(report.unreadable)} entr
            {report.unreadable === 1 ? "y" : "ies"} could not be read and
            will be left as they are.
          </p>
        )}

        <div className="overflow-hidden rounded-[5px] border border-line-soft">
          <table className="w-full border-collapse text-left font-mono">
            <tbody>
              {report.sample.map((row, i) => (
                <tr
                  key={i}
                  className="border-b border-line-soft/60 last:border-0"
                >
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
          <p className="-mt-3 text-[13px] text-fg-dim">
            …and {formatCount(report.toRename - report.sample.length)} more,
            shown the same way.
          </p>
        )}

        <Label
          htmlFor="backup-confirmed"
          className="items-start gap-2.5 rounded-[5px] border border-line bg-raised px-3 py-2.5 text-[14px] text-fg"
        >
          <Checkbox
            id="backup-confirmed"
            checked={confirmed}
            onCheckedChange={(checked) => onConfirmedChange(checked === true)}
            className="mt-0.5"
          />
          <span>I have a backup of this folder somewhere else.</span>
        </Label>

        {error && <p className="text-danger">{error}</p>}

        <div className="flex items-center gap-2">
          <Button size="lg" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button
            size="lg"
            variant="accent"
            className="ml-auto"
            onClick={onImport}
            disabled={!confirmed || busy}
          >
            {busy ? "Importing…" : "Import"}
          </Button>
        </div>
      </div>
    </div>
  );
}
