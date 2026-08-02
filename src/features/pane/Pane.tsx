/**
 * The pane: the right half of the split, and the single most reused surface
 * in the app.
 *
 * Drag-resizable (the handle is the shell's, since it sits between the two
 * halves), fully closable, widths remembered per mode. A labelled three-way
 * control in its own header switches what it holds.
 *
 * **There is no theatre view.** Full-window is the pane maximised — one
 * control, one state, no transition to design and no scroll position to
 * restore.
 *
 * M2.5a builds Preview. Grid and Folders are M2.5b's, and the control shows
 * them as coming rather than hiding them: the shape of the pane is a decision
 * already made, and a control that grows a third option later is a worse
 * surprise than one that starts complete.
 */

import { IconButton } from "../../components/Button";
import { Tooltip } from "../../components/Tooltip";
import type { GridItem } from "../../lib/types";
import { PANE_MODES, type PaneMode } from "../../state/ui";
import { PreviewMode, type PreviewSlot } from "./PreviewMode";

export interface PaneProps {
  mode: PaneMode;
  onModeChange: (mode: PaneMode) => void;
  onClose: () => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;

  slots: PreviewSlot[];
  items: GridItem[];
  thumbsDir: string;
  onStep: (delta: number) => void;
  onPick: (itemId: number) => void;
  detailsExpanded: boolean;
  onDetailsExpandedChange: (expanded: boolean) => void;
  refreshToken: number;
}

export function Pane({
  mode,
  onModeChange,
  onClose,
  maximised,
  onMaximisedChange,
  slots,
  items,
  thumbsDir,
  onStep,
  onPick,
  detailsExpanded,
  onDetailsExpandedChange,
  refreshToken,
}: PaneProps) {
  return (
    <section
      aria-label="Pane"
      className="flex min-h-0 min-w-0 flex-1 flex-col border-l border-line bg-panel"
    >
      <header className="flex shrink-0 items-center gap-1 border-b border-line px-1.5 py-1">
        <div role="tablist" aria-label="Pane mode" className="flex items-center gap-0.5">
          {PANE_MODES.map((option) => {
            const ready = option.key === "preview";
            const button = (
              <button
                key={option.key}
                role="tab"
                type="button"
                aria-selected={mode === option.key}
                disabled={!ready}
                onClick={() => onModeChange(option.key)}
                className={`rounded-[4px] px-2 py-[3px] text-[12px] ${
                  mode === option.key
                    ? "bg-accent/15 text-accent"
                    : "text-fg-mid hover:bg-hover hover:text-fg"
                } ${ready ? "" : "opacity-35"}`}
              >
                {option.label}
              </button>
            );
            return ready ? (
              button
            ) : (
              <Tooltip
                key={option.key}
                side="bottom"
                label={`${option.label} arrives with the sorting surfaces`}
              >
                <span>{button}</span>
              </Tooltip>
            );
          })}
        </div>

        <span className="ml-auto flex items-center gap-0.5">
          <Tooltip
            side="bottom"
            label={maximised ? "Back to the split" : "Fill the window"}
          >
            <IconButton
              aria-label={maximised ? "Back to the split" : "Fill the window"}
              active={maximised}
              onClick={() => onMaximisedChange(!maximised)}
            >
              {maximised ? "⤡" : "⤢"}
            </IconButton>
          </Tooltip>
          <Tooltip side="bottom" label="Close the pane">
            <IconButton aria-label="Close the pane" onClick={onClose}>
              ✕
            </IconButton>
          </Tooltip>
        </span>
      </header>

      <div className="min-h-0 flex-1">
        {mode === "preview" && (
          <PreviewMode
            slots={slots}
            items={items}
            thumbsDir={thumbsDir}
            onStep={onStep}
            onPick={onPick}
            detailsExpanded={detailsExpanded}
            onDetailsExpandedChange={onDetailsExpandedChange}
            refreshToken={refreshToken}
          />
        )}
      </div>
    </section>
  );
}
