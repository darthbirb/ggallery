/** Modal dialogs over Radix, styled to the app. Focus trapping, Escape and
 *  the scroll lock come from the primitive; the surface is ours. */

import * as RadixDialog from "@radix-ui/react-dialog";
import type { ReactNode } from "react";

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = 460,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  return (
    <RadixDialog.Root open={open} onOpenChange={onOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="overlay-in fixed inset-0 z-40 bg-black/55" />
        <RadixDialog.Content
          style={{ width }}
          className="surface-in fixed left-1/2 top-1/2 z-50 max-h-[86vh] -translate-x-1/2 -translate-y-1/2 overflow-y-auto rounded-[6px] border border-line bg-panel shadow-[0_24px_60px_rgba(0,0,0,0.55)]"
        >
          <header className="flex items-start gap-3 border-b border-line px-4 py-3">
            <div className="min-w-0 flex-1">
              <RadixDialog.Title className="text-[14px] font-semibold text-fg">
                {title}
              </RadixDialog.Title>
              {description && (
                <RadixDialog.Description className="mt-0.5 text-[12px] text-fg-mid">
                  {description}
                </RadixDialog.Description>
              )}
            </div>
            <RadixDialog.Close className="rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg">
              ✕
            </RadixDialog.Close>
          </header>

          {children && <div className="px-4 py-3">{children}</div>}

          {footer && (
            <footer className="flex items-center justify-end gap-2 border-t border-line px-4 py-3">
              {footer}
            </footer>
          )}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}

/**
 * The named confirmation destructive operations need. It always says what
 * will happen to what, by name — never "Are you sure?".
 */
export function ConfirmDialog({
  open,
  onOpenChange,
  title,
  body,
  confirmLabel,
  danger,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  body: ReactNode;
  confirmLabel: string;
  danger?: boolean;
  onConfirm: () => void;
}) {
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={title}
      width={420}
      footer={
        <>
          <button
            type="button"
            onClick={() => onOpenChange(false)}
            className="rounded-[4px] border border-line px-3 py-1 text-fg-mid hover:bg-hover hover:text-fg"
          >
            Cancel
          </button>
          <button
            type="button"
            autoFocus
            onClick={() => {
              onOpenChange(false);
              onConfirm();
            }}
            className={
              danger
                ? "rounded-[4px] border border-danger/60 bg-danger/15 px-3 py-1 text-danger hover:bg-danger/25"
                : "rounded-[4px] border border-accent-d bg-accent/15 px-3 py-1 text-accent hover:bg-accent/25"
            }
          >
            {confirmLabel}
          </button>
        </>
      }
    >
      <p className="text-[13px] text-fg-mid">{body}</p>
    </Dialog>
  );
}
