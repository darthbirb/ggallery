/**
 * Settings — one window, with a section list down the left rather than a
 * chain of windows that each replace the last.
 *
 * It used to be four: this dialog, plus Archetypes, Statuses and Tags each
 * opening as their own full `<Dialog>` that closed Settings to open. That
 * meant no way back to the list short of closing and reopening the whole
 * thing, and every switch between them unmounted one dialog and mounted
 * another. One dialog, one section list, content swapping underneath it —
 * `ArchetypesSection`, `StatusesSection` and `TagsSection` are the same
 * editors, now sections rather than screens.
 *
 * Still deliberately small — the full screen (command palette, keyboard
 * reference, blur toggle, backup verification) is M9's. What is here is what
 * earlier milestones made mandatory: the repair-case rename action (M1.6),
 * the archetype, status and tag editors that shipping no vocabulary requires
 * (M2.1, decision 21), and the accent, which is a preference and nothing else.
 *
 * **Panel widths are not here.** M2.5a put sliders for them in this dialog on
 * the strength of a DESIGN.md line about being editable "not only by
 * dragging"; in use they were a number to type at a thing you had already got
 * right with the mouse. Dragging an edge sizes a panel, double-clicking it
 * resets — that is the whole interaction, and DESIGN.md now says so.
 */

import { useState } from "react";

import { Dialog } from "../../components/Dialog";
import { cn } from "../../lib/utils";
import { ACCENTS, useUi } from "../../state/ui";
import { ArchetypesSection } from "./ArchetypesSection";
import { StatusesSection } from "./StatusesSection";
import { TagsSection } from "./TagsSection";

type Section = "general" | "archetypes" | "statuses" | "tags";

const SECTIONS: { key: Section; label: string }[] = [
  { key: "general", label: "General" },
  { key: "archetypes", label: "Archetypes" },
  { key: "statuses", label: "Folder statuses" },
  { key: "tags", label: "Tags" },
];

export function SettingsPanel({
  onClose,
  onNormaliseFilenames,
  onArchetypesChanged,
  onStatusesChanged,
  onTagsChanged,
}: {
  onClose: () => void;
  onNormaliseFilenames: () => void;
  onArchetypesChanged: () => void;
  onStatusesChanged: () => void;
  onTagsChanged: () => void;
}) {
  const [section, setSection] = useState<Section>("general");

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} title="Settings" width={720}>
      <div className="flex min-h-[420px] gap-4">
        <nav className="flex w-[170px] shrink-0 flex-col gap-0.5 border-r border-line pr-3">
          {SECTIONS.map((candidate) => (
            <button
              key={candidate.key}
              type="button"
              aria-current={section === candidate.key}
              onClick={() => setSection(candidate.key)}
              className={cn(
                "flex h-8 items-center rounded-[4px] px-2.5 text-left",
                section === candidate.key
                  ? "bg-accent/15 text-accent"
                  : "text-fg-mid hover:bg-hover hover:text-fg",
              )}
            >
              {candidate.label}
            </button>
          ))}
        </nav>

        <div className="min-w-0 flex-1 overflow-y-auto">
          {section === "general" && (
            <GeneralSection onNormaliseFilenames={onNormaliseFilenames} />
          )}
          {section === "archetypes" && (
            <ArchetypesSection onChanged={onArchetypesChanged} />
          )}
          {section === "statuses" && <StatusesSection onChanged={onStatusesChanged} />}
          {section === "tags" && <TagsSection onChanged={onTagsChanged} />}
        </div>
      </div>
    </Dialog>
  );
}

function GeneralSection({ onNormaliseFilenames }: { onNormaliseFilenames: () => void }) {
  const ui = useUi();

  return (
    <>
      <SectionHeading>Accent</SectionHeading>
      {/* A grid, not a wrapping row: six chips of different label widths
          left themselves a ragged right edge and the block read as
          off-centre in the dialog. Three equal columns fill the width. */}
      <div role="radiogroup" aria-label="Accent" className="grid grid-cols-3 gap-2">
        {ACCENTS.map((accent) => (
          <button
            key={accent.key}
            type="button"
            role="radio"
            aria-checked={ui.accent === accent.key}
            onClick={() => ui.set("accent", accent.key)}
            // Scopes `--color-accent` to this button, so the swatch paints
            // itself in its own hue rather than the one currently chosen —
            // see the `[data-accent]` rules in `styles/index.css`.
            data-accent={accent.key}
            className={cn(
              "flex h-8 w-full items-center gap-2 rounded-[4px] border bg-raised px-2.5 text-[13px]",
              ui.accent === accent.key
                ? "border-accent text-fg"
                : "border-line text-fg-mid hover:bg-hover hover:text-fg",
            )}
          >
            <span className="size-3.5 rounded-full border border-accent-d bg-accent" />
            {accent.label}
          </button>
        ))}
      </div>

      <SectionHeading>Library</SectionHeading>
      <Action
        title="Normalise filenames"
        body="Rename anything that lost its UUID name."
        onClick={onNormaliseFilenames}
      />
    </>
  );
}

function SectionHeading({ children }: { children: React.ReactNode }) {
  return (
    <h3 className="mb-2 mt-4 font-mono uppercase tracking-[0.1em] text-fg-dim first:mt-0">
      {children}
    </h3>
  );
}

function Action({
  title,
  body,
  onClick,
}: {
  title: string;
  body: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex w-full items-center gap-3 rounded-[5px] border border-line bg-raised px-3 py-2 text-left hover:border-fg-dim hover:bg-hover"
    >
      <span className="min-w-0 flex-1">
        <span className="block text-fg">{title}</span>
        <span className="block text-[13px] text-fg-dim">{body}</span>
      </span>
    </button>
  );
}
