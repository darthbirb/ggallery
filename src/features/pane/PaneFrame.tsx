/**
 * The part of the pane every mode shares: one header row ending in maximise
 * and close, and the body beneath it.
 *
 * The mode fills the rest of the header with whatever names what it is
 * showing — for Preview that is the item's filename and size, which is why
 * there is no separate "Preview" tab. A labelled control naming the only mode
 * that exists was a label, not a choice; M2.5b brings back a real switcher
 * when there is something to switch between, and it goes in the same slot.
 *
 * It lives in its own file rather than in `Pane.tsx` because the modes import
 * it and `Pane` imports the modes — together in one file that is a cycle,
 * which happens to work under Vite today and is not worth relying on.
 */

import { Maximize2, Minimize2, X } from "lucide-react";
import type { ReactNode } from "react";

import { Tooltip } from "../../components/Tooltip";
import { IconButton } from "../../components/ui/button";

export interface PaneFrameProps {
  /** What this mode puts in the header, left of the window controls. */
  header?: ReactNode;
  maximised: boolean;
  onMaximisedChange: (maximised: boolean) => void;
  onClose: () => void;
  children: ReactNode;
}

export function PaneFrame({
  header,
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
      <header className="flex h-11 shrink-0 items-center gap-2 border-b border-line px-2">
        {header ?? <span className="flex-1" />}

        <span className="flex shrink-0 items-center gap-1">
          <Tooltip
            side="bottom"
            label={maximised ? "Back to the split" : "Fill the window"}
          >
            <IconButton
              aria-label={maximised ? "Back to the split" : "Fill the window"}
              active={maximised}
              onClick={() => onMaximisedChange(!maximised)}
            >
              {maximised ? <Minimize2 /> : <Maximize2 />}
            </IconButton>
          </Tooltip>
          <Tooltip side="bottom" label="Close the pane">
            <IconButton aria-label="Close the pane" onClick={onClose}>
              <X />
            </IconButton>
          </Tooltip>
        </span>
      </header>

      {children}
    </section>
  );
}
