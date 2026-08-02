/**
 * The toast queue — and, until M4 builds the `Ctrl+Z` stack, the only visible
 * path to the undo journal at all.
 *
 * Locked decision 23: every destructive action ends in a toast naming what
 * happened, with an Undo button. Not decoration — M2.1 shipped journalled
 * moves and deletes with no mouse path to reverse them, and a journal nothing
 * points at is a journal nobody knows exists.
 *
 * The rendering lives in `components/Toaster.tsx`; this is the queue.
 */

import {
  createContext,
  createElement,
  useCallback,
  useContext,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

export interface Toast {
  id: number;
  /** What happened, in the past tense, naming the thing and the destination:
   *  "Moved 4 items to Trips", not "Move complete". */
  message: string;
  tone: "neutral" | "danger";
  /** Absent for a toast with nothing to reverse — a failure, say. */
  undo?: () => Promise<void>;
  /** Shown instead of the message once Undo has been pressed and finished. */
  undoneMessage?: string;
  undone?: boolean;
  /** Set when the undo itself failed, and shown in its place. */
  error?: string;
}

export interface ToastOptions {
  message: string;
  tone?: Toast["tone"];
  undo?: () => Promise<void>;
  undoneMessage?: string;
}

export interface ToastQueue {
  toasts: Toast[];
  push: (options: ToastOptions) => void;
  dismiss: (id: number) => void;
  /** Runs the toast's undo and rewrites it in place, so the same strip says
   *  what happened rather than stacking a second message under the first. */
  runUndo: (id: number) => Promise<void>;
}

function useToastQueue(): ToastQueue {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const push = useCallback((options: ToastOptions) => {
    const id = nextId.current++;
    setToasts((current) => [
      ...current,
      {
        id,
        message: options.message,
        tone: options.tone ?? "neutral",
        undo: options.undo,
        undoneMessage: options.undoneMessage,
      },
    ]);
  }, []);

  const dismiss = useCallback((id: number) => {
    setToasts((current) => current.filter((toast) => toast.id !== id));
  }, []);

  const runUndo = useCallback(async (id: number) => {
    const toast = toastsRef.current.find((candidate) => candidate.id === id);
    if (!toast?.undo) return;
    try {
      await toast.undo();
      setToasts((current) =>
        current.map((candidate) =>
          candidate.id === id
            ? { ...candidate, undone: true, undo: undefined, error: undefined }
            : candidate,
        ),
      );
    } catch (err) {
      setToasts((current) =>
        current.map((candidate) =>
          candidate.id === id
            ? {
                ...candidate,
                error: err instanceof Error ? err.message : String(err),
              }
            : candidate,
        ),
      );
    }
  }, []);

  // `runUndo` is handed to a button that outlives the render it was made in;
  // reading the list through a ref keeps it stable without going stale.
  const toastsRef = useRef(toasts);
  toastsRef.current = toasts;

  return useMemo(
    () => ({ toasts, push, dismiss, runUndo }),
    [toasts, push, dismiss, runUndo],
  );
}

const ToastContext = createContext<ToastQueue | null>(null);

export function ToastProvider({ children }: { children: ReactNode }) {
  const value = useToastQueue();
  return createElement(ToastContext.Provider, { value }, children);
}

export function useToasts(): ToastQueue {
  const value = useContext(ToastContext);
  if (!value) throw new Error("useToasts must be used inside <ToastProvider>");
  return value;
}
