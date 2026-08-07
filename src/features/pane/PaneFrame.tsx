/**
 * The part of the pane every mode shares: one header row starting with the
 * fill-window arrow, the mode switcher and the fold-to-strip toggle at its
 * right end, and the body beneath it.
 *
 * The mode fills the rest of the header with whatever names what it is
 * showing — for Preview that is dimensions and file size, which is why there
 * is no separate "Preview" label. The header's right end mirrors the
 * navigation panel's own chrome: a fold control that collapses this panel to
 * a strip of the same icons rather than dismissing it (`PaneStrip`, in
 * `Pane.tsx`, is that strip) — always the rightmost control, so it never
 * trades places with a mode-dependent one — and the three mode buttons stay
 * in the same order and position whatever else is in the header; unbuilt
 * modes (M2.5b) sit here inert rather than hidden, the same call the nav's
 * own placeholder pair makes.
 *
 * It lives in its own file rather than in `Pane.tsx` because the modes import
 * it and `Pane` imports the modes — together in one file that is a cycle,
 * which happens to work under Vite today and is not worth relying on. The
 * mode icons live in `modeIcons.ts` for the same reason.
 */

import { ArrowLeftToLine, ArrowRightToLine, PanelRightClose } from "lucide-react";
import type { ReactNode } from "react";

import { Tooltip } from "../../components/Tooltip";
import { IconButton } from "../../components/ui/button";
import { AVAILABLE_PANE_MODES, PANE_MODES, type PaneMode } from "../../state/ui";
import { PANE_MODE_ICONS } from "./modeIcons";

export interface PaneFrameProps {
  /** What this mode puts in the header, between the fill-window arrow and
   *  the mode switcher. */
  header?: ReactNode;
  mode: PaneMode;
  onModeChange: (mode: PaneMode) => void;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;
  onClose: () => void;
  children: ReactNode;
}

export function PaneFrame({
  header,
  mode,
  onModeChange,
  maximised,
  onMaximisedChange,
  onClose,
  children,
}: PaneFrameProps) {
  return (
    <section
      aria-label="Pane"
      className="flex min-h-0 min-w-0 flex-1 flex-col border-l border-line bg-panel"
    >
      <header className="flex h-11 shrink-0 items-center gap-1 border-b border-line px-2">
        {/* The one control today that a mode needs beyond the frame's own —
            Preview's fullscreen. Generic to the frame rather than passed in
            per mode, since it acts on the pane as a whole either way. Left
            of the header, beside Details — the other control that changes
            how much of the pane you see — rather than with maximise and
            close, which change whether the pane is there at all. The arrow
            points the direction the pane will actually travel, which only
            reads from the edge it grows from. */}
        <Tooltip side="bottom" label={maximised ? "Back To The Split" : "Fill The Window"}>
          <IconButton
            aria-label={maximised ? "Back To The Split" : "Fill The Window"}
            active={maximised}
            onClick={() => onMaximisedChange(!maximised)}
          >
            {maximised ? <ArrowRightToLine /> : <ArrowLeftToLine />}
          </IconButton>
        </Tooltip>

        {header ?? <span className="flex-1" />}

        <span className="flex shrink-0 items-center gap-1">
          {PANE_MODES.map((candidate) => {
            const Icon = PANE_MODE_ICONS[candidate.key];
            const built = AVAILABLE_PANE_MODES.some((option) => option.key === candidate.key);
            return (
              <Tooltip key={candidate.key} side="bottom" label={candidate.label}>
                <IconButton
                  aria-label={candidate.label}
                  active={mode === candidate.key}
                  onClick={built ? () => onModeChange(candidate.key) : () => {}}
                >
                  <Icon />
                </IconButton>
              </Tooltip>
            );
          })}
        </span>

        {/* Rightmost, always — the one control that never shares its
            position with a mode-dependent one. */}
        <Tooltip side="bottom" label="Hide The Pane">
          <IconButton aria-label="Hide The Pane" onClick={onClose}>
            <PanelRightClose />
          </IconButton>
        </Tooltip>
      </header>

      {children}
    </section>
  );
}
