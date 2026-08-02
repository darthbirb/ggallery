/**
 * Where toast-and-undo becomes visible. The queue is `state/toasts.ts`.
 *
 * A toast names what happened and offers Undo. Pressing it rewrites the same
 * toast in place — "Moved 4 items to Trips" becomes "Move undone" — rather
 * than stacking a second message about the first.
 */

import * as RadixToast from "@radix-ui/react-toast";
import type { ReactNode } from "react";

import { useToasts } from "../state/toasts";

/** Long enough to read a sentence and reach the button without hurrying;
 *  toasts with nothing to undo go sooner. */
const WITH_UNDO_MS = 9000;
const PLAIN_MS = 4500;

export function ToastProviderRoot({ children }: { children: ReactNode }) {
  return (
    <RadixToast.Provider swipeDirection="right">
      {children}
      <RadixToast.Viewport className="fixed bottom-3 right-3 z-[60] flex w-[380px] max-w-[calc(100vw-24px)] flex-col gap-2 outline-none" />
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
          className="surface-in flex items-center gap-3 rounded-[5px] border border-line bg-panel px-3 py-2 shadow-[0_12px_32px_rgba(0,0,0,0.5)] data-[swipe=move]:translate-x-[var(--radix-toast-swipe-move-x)]"
        >
          <RadixToast.Title
            className={`min-w-0 flex-1 truncate text-[13px] ${
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
              altText="Undo"
              onClick={(event) => {
                // The toast stays up while the undo runs, and reports what
                // happened in place — closing here would hide a failure.
                event.preventDefault();
                void runUndo(toast.id);
              }}
              className="shrink-0 rounded-[4px] border border-accent-d bg-accent/15 px-2 py-0.5 text-[12px] text-accent hover:bg-accent/25"
            >
              Undo
            </RadixToast.Action>
          )}

          <RadixToast.Close
            aria-label="Dismiss"
            className="shrink-0 rounded-[3px] px-1 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </RadixToast.Close>
        </RadixToast.Root>
      ))}
    </>
  );
}
