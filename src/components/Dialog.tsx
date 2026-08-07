/**
 * The one dialog shape the app uses, composed from shadcn/ui's dialog parts.
 *
 * Title, optional description, body, footer — every modal in the app is this,
 * so the header sits at the same height and the buttons in the same corner
 * whatever is being asked. Focus trapping, Escape and the scroll lock come
 * from Radix underneath.
 */

import type { ReactNode } from "react";

import {
  Dialog as DialogRoot,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogOverlay,
  DialogPortal,
  DialogTitle,
} from "./ui/dialog";
import { Button } from "./ui/button";

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  footer,
  width = 480,
  closable = true,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** False while an operation is mid-flight and abandoning it would leave
   *  the library half-changed. Escape, the overlay and the close button all
   *  stop working together, rather than two of the three. */
  closable?: boolean;
}) {
  return (
    <DialogRoot
      open={open}
      onOpenChange={(next) => {
        if (!next && !closable) return;
        onOpenChange(next);
      }}
    >
      <DialogPortal>
        <DialogOverlay />
        <DialogContent
          style={{ width }}
          onEscapeKeyDown={(event) => !closable && event.preventDefault()}
          onPointerDownOutside={(event) => !closable && event.preventDefault()}
          onInteractOutside={(event) => !closable && event.preventDefault()}
        >
          <DialogHeader>
            <div className="min-w-0 flex-1">
              <DialogTitle className="text-16 font-semibold text-fg">
                {title}
              </DialogTitle>
              {description && (
                <DialogDescription className="mt-0.5 text-13 text-fg-mid">
                  {description}
                </DialogDescription>
              )}
            </div>
            {closable && <DialogClose />}
          </DialogHeader>

          {children && <div className="min-h-0 overflow-y-auto px-4 py-3">{children}</div>}

          {footer && <DialogFooter>{footer}</DialogFooter>}
        </DialogContent>
      </DialogPortal>
    </DialogRoot>
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
      width={440}
      footer={
        <>
          <Button onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button
            autoFocus
            variant={danger ? "danger" : "accent"}
            onClick={() => {
              onOpenChange(false);
              onConfirm();
            }}
          >
            {confirmLabel}
          </Button>
        </>
      }
    >
      <p className="text-14 leading-relaxed text-fg-mid">{body}</p>
    </Dialog>
  );
}
