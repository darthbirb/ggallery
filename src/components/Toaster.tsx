/**
 * Where toast-and-undo becomes visible. The queue is `state/toasts.ts`.
 *
 * A toast names what happened and offers Undo. Pressing it rewrites the same
 * toast in place — "Moved 4 items to Trips" becomes "Move undone" — rather
 * than stacking a second message about the first.
 *
 * Undo is a real button on the primitives, not a link: this is the visible
 * path to the journal that locked decision 23 requires, so it has to look
 * like something you can press.
 */

import * as RadixToast from "@radix-ui/react-toast";
import { X } from "lucide-react";
import type { ReactNode } from "react";

import { useToasts } from "../state/toasts";
import { Button } from "./ui/button";

/** Long enough to read a sentence and reach the button without hurrying;
 *  toasts with nothing to undo go sooner. */
const WITH_UNDO_MS = 9000;
const PLAIN_MS = 4500;

export function ToastProviderRoot({ children }: { children: ReactNode }) {
  return (
    <RadixToast.Provider swipeDirection="right">
      {children}
      <RadixToast.Viewport className="fixed bottom-3 right-3 z-[60] flex w-[400px] max-w-[calc(100vw-24px)] flex-col gap-2 outline-none" />
    </RadixToast.Provider>
  );
}

export function Toaster() {
  const { toasts, dismiss, runUndo } = useToasts();

  return (
    <>
      {toasts.map((toast) => (
        <RadixToast.Root
          key={toast.id}
          duration={toast.undo ? WITH_UNDO_MS : PLAIN_MS}
          onOpenChange={(open) => {
            if (!open) dismiss(toast.id);
          }}
          className="surface-in flex items-center gap-2.5 rounded-[6px] border border-line bg-panel px-3 py-2 shadow-[0_16px_40px_rgba(0,0,0,0.55)] data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)]"
        >
          <RadixToast.Title
            className={`min-w-0 flex-1 truncate text-14 ${
              toast.tone === "danger" && !toast.undone ? "text-danger" : "text-fg"
            }`}
          >
            {toast.error
              ? `Could not undo: ${toast.error}`
              : toast.undone
                ? (toast.undoneMessage ?? "Undone.")
                : toast.message}
          </RadixToast.Title>

          {toast.undo && (
            <RadixToast.Action
              asChild
              altText="Undo"
              onClick={(event) => {
                // The toast stays up while the undo runs, and reports what
                // happened in place — closing here would hide a failure.
                event.preventDefault();
                void runUndo(toast.id);
              }}
            >
              <Button variant="accent" size="sm">
                Undo
              </Button>
            </RadixToast.Action>
          )}

          {/* The drawing's toast dismiss is a 24px sub-control with no
              surface, not a full icon button — it sits beside Undo, which is
              the control the toast is actually offering, and a second framed
              button next to it competes for that. */}
          <RadixToast.Close asChild>
            <Button variant="subtle" size="sub-lg" aria-label="Dismiss">
              <X />
            </Button>
          </RadixToast.Close>
        </RadixToast.Root>
      ))}
    </>
  );
}
