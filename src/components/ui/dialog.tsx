/** shadcn/ui's dialog parts, restyled. Focus trapping, Escape and the scroll
 *  lock come from Radix; the surface is ours. `components/Dialog.tsx` composes
 *  these into the one dialog shape the app actually uses. */

import * as DialogPrimitive from "@radix-ui/react-dialog";
import { X } from "lucide-react";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";
import { IconButton } from "./button";

export const Dialog = DialogPrimitive.Root;
export const DialogTrigger = DialogPrimitive.Trigger;
export const DialogPortal = DialogPrimitive.Portal;
export const DialogTitle = DialogPrimitive.Title;
export const DialogDescription = DialogPrimitive.Description;

export function DialogOverlay({
  className,
  ...rest
}: ComponentProps<typeof DialogPrimitive.Overlay>) {
  return (
    <DialogPrimitive.Overlay
      data-slot="dialog-overlay"
      // No `overlay-in` fade here. One dialog routinely replaces another
      // (Settings → Archetypes, say) — a fresh overlay unmounts and a new
      // one mounts in the same commit, and if the new one fades in from
      // transparent, the screen visibly *undims* for the length of that fade
      // before dimming again. The backdrop snapping to its final state is
      // unremarkable; the panel's own `surface-in` still gives every dialog
      // a pop.
      className={cn("fixed inset-0 z-40 bg-black/60", className)}
      {...rest}
    />
  );
}

export function DialogContent({
  className,
  children,
  ...rest
}: ComponentProps<typeof DialogPrimitive.Content>) {
  return (
    <DialogPrimitive.Content
      data-slot="dialog-content"
      className={cn(
        "surface-in fixed left-1/2 top-1/2 z-50 flex max-h-[86vh] -translate-x-1/2 -translate-y-1/2 flex-col",
        "overflow-hidden rounded-[8px] border border-line bg-panel text-14 text-fg",
        "shadow-[0_28px_70px_rgba(0,0,0,0.6)]",
        // Radix focuses the panel when a dialog opens; a ring around the
        // whole thing is not what "where am I" should mean. The controls
        // inside still get one — see `styles/index.css`.
        "outline-none",
        className,
      )}
      {...rest}
    >
      {children}
    </DialogPrimitive.Content>
  );
}

/** The close affordance, as a real button with a surface — not a bare glyph
 *  in the corner (decision 25). */
export function DialogClose({ className }: { className?: string }) {
  return (
    <DialogPrimitive.Close asChild>
      <IconButton aria-label="Close" className={className}>
        <X />
      </IconButton>
    </DialogPrimitive.Close>
  );
}

export function DialogHeader({ className, ...rest }: ComponentProps<"header">) {
  return (
    <header
      data-slot="dialog-header"
      className={cn(
        "flex shrink-0 items-start gap-3 border-b border-line px-4 py-3",
        className,
      )}
      {...rest}
    />
  );
}

export function DialogFooter({ className, ...rest }: ComponentProps<"footer">) {
  return (
    <footer
      data-slot="dialog-footer"
      className={cn(
        "flex shrink-0 items-center justify-end gap-2 border-t border-line px-4 py-3",
        className,
      )}
      {...rest}
    />
  );
}
