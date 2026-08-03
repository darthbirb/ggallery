/** shadcn/ui's tooltip, restyled. Icon-only controls carry one; the folded
 *  navigation panel is entirely icon-only, so this is what keeps it readable. */

import * as TooltipPrimitive from "@radix-ui/react-tooltip";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

export function TooltipProvider({
  delayDuration = 400,
  ...rest
}: ComponentProps<typeof TooltipPrimitive.Provider>) {
  return (
    <TooltipPrimitive.Provider
      data-slot="tooltip-provider"
      delayDuration={delayDuration}
      skipDelayDuration={300}
      {...rest}
    />
  );
}

export const TooltipRoot = TooltipPrimitive.Root;
export const TooltipTrigger = TooltipPrimitive.Trigger;

export function TooltipContent({
  className,
  sideOffset = 6,
  children,
  ...rest
}: ComponentProps<typeof TooltipPrimitive.Content>) {
  return (
    <TooltipPrimitive.Portal>
      <TooltipPrimitive.Content
        data-slot="tooltip-content"
        sideOffset={sideOffset}
        collisionPadding={8}
        className={cn(
          "surface-in z-50 max-w-[36ch] rounded-[4px] border border-line bg-raised px-2 py-1",
          "text-[13px] text-fg shadow-[0_8px_20px_rgba(0,0,0,0.45)]",
          className,
        )}
        {...rest}
      >
        {children}
      </TooltipPrimitive.Content>
    </TooltipPrimitive.Portal>
  );
}
