/**
 * Settings.
 *
 * Still deliberately small — the full screen (command palette, keyboard
 * reference, blur toggle, backup verification) is M9's. What is here is what
 * earlier milestones made mandatory: the repair-case rename action (M1.6), the
 * archetype, status and tag editors that shipping no vocabulary requires
 * (M2.1, decision 21), and now the interface preferences M2.5a introduces —
 * the accent, and the panel widths, which docs/DESIGN.md §2 says must be
 * editable here and not only by dragging.
 */

import { Dialog } from "../../components/Dialog";
import { Slider } from "../../components/Slider";
import {
  ACCENTS,
  NAV_DEFAULT,
  NAV_MAX,
  NAV_MIN,
  PANE_DEFAULT,
  PANE_MIN,
  PANE_MODES,
  useUi,
} from "../../state/ui";

export function SettingsPanel({
  onClose,
  onNormaliseFilenames,
  onManageArchetypes,
  onManageStatuses,
  onManageTags,
}: {
  onClose: () => void;
  onNormaliseFilenames: () => void;
  onManageArchetypes: () => void;
  onManageStatuses: () => void;
  onManageTags: () => void;
}) {
  const ui = useUi();

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()} title="Settings" width={520}>
      <Section title="Appearance">
        <p className="mb-2 text-[12px] text-fg-dim">
          One accent carries selection, focus, the active tab, drop acceptance
          and the scrubber. Green and red stay reserved for meaning.
        </p>
        <div
          role="radiogroup"
          aria-label="Accent"
          className="flex flex-wrap items-center gap-1.5"
        >
          {ACCENTS.map((accent) => (
            <button
              key={accent.key}
              type="button"
              role="radio"
              aria-checked={ui.accent === accent.key}
              onClick={() => ui.set("accent", accent.key)}
              data-accent={accent.key}
              className={`flex items-center gap-1.5 rounded-[4px] border px-2 py-1 text-[12px] ${
                ui.accent === accent.key
                  ? "border-accent text-fg"
                  : "border-line text-fg-mid hover:bg-hover"
              }`}
            >
              <span className="h-3 w-3 rounded-full bg-accent" />
              {accent.label}
            </button>
          ))}
        </div>
      </Section>

      <Section title="Panels">
        <WidthRow
          label="Navigation panel"
          value={ui.navWidth}
          min={NAV_MIN}
          max={NAV_MAX}
          onChange={(width) => ui.set("navWidth", width)}
          onReset={ui.resetNavWidth}
          defaultValue={NAV_DEFAULT}
        />

        {PANE_MODES.map((mode) => (
          <WidthRow
            key={mode.key}
            label={`Pane — ${mode.label}`}
            value={ui.paneWidths[mode.key]}
            min={PANE_MIN}
            max={1200}
            onChange={(width) => ui.setPaneWidth(mode.key, width)}
            onReset={() => ui.setPaneWidth(mode.key, PANE_DEFAULT[mode.key])}
            defaultValue={PANE_DEFAULT[mode.key]}
          />
        ))}

        <p className="mt-1 text-[12px] text-fg-dim">
          Widths are remembered per pane mode. Dragging a panel edge does the
          same thing; double-clicking one resets it.
        </p>
      </Section>

      <Section title="Library">
        <Action
          title="Normalise filenames"
          body="Find any file that is not UUID-named and rename it — for when something outside the app has renamed a file back."
          onClick={onNormaliseFilenames}
        />
      </Section>

      <Section title="Vocabulary">
        <Action
          title="Archetypes"
          body="Create and edit the folder templates used across your library."
          onClick={onManageArchetypes}
        />
        <Action
          title="Folder statuses"
          body="Rename, recolour, reorder, add or remove status values."
          onClick={onManageStatuses}
        />
        <Action
          title="Tags"
          body="Rename or delete a tag across the whole library."
          onClick={onManageTags}
        />
      </Section>
    </Dialog>
  );
}

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="mb-4 last:mb-0">
      <h3 className="mb-2 font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
        {title}
      </h3>
      {children}
    </section>
  );
}

function WidthRow({
  label,
  value,
  min,
  max,
  onChange,
  onReset,
  defaultValue,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (width: number) => void;
  onReset: () => void;
  defaultValue: number;
}) {
  return (
    <div className="mb-1.5 flex items-center gap-3">
      <span className="w-[150px] shrink-0 truncate text-fg-mid">{label}</span>
      <Slider
        label={label}
        value={Math.min(Math.max(value, min), max)}
        min={min}
        max={max}
        step={2}
        onChange={onChange}
        className="min-w-0 flex-1"
      />
      <span className="w-14 shrink-0 text-right font-mono text-[11px] tabular-nums text-fg-dim">
        {Math.round(value)}px
      </span>
      <button
        type="button"
        onClick={onReset}
        disabled={Math.round(value) === defaultValue}
        className="shrink-0 text-[11px] text-fg-dim hover:text-fg disabled:opacity-30"
      >
        reset
      </button>
    </div>
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
      className="mb-2 w-full rounded-[4px] border border-line px-3 py-2 text-left last:mb-0 hover:bg-hover"
    >
      <span className="block text-fg">{title}</span>
      <span className="block text-[12px] text-fg-dim">{body}</span>
    </button>
  );
}
