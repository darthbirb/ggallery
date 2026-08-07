/** shadcn/ui's select, restyled. A native `<select>` renders with the Windows
 *  system palette inside the WebView — a white popup in a dark application —
 *  which is precisely the unfinished look this pass exists to remove. */

import * as SelectPrimitive from "@radix-ui/react-select";
import { Check, ChevronDown } from "lucide-react";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";
import { menuItem, menuSurface } from "./menu";

export const Select = SelectPrimitive.Root;
export const SelectValue = SelectPrimitive.Value;

export function SelectTrigger({
  className,
  children,
  ...rest
}: ComponentProps<typeof SelectPrimitive.Trigger>) {
  return (
    <SelectPrimitive.Trigger
      data-slot="select-trigger"
      className={cn(
        "flex h-8 w-full items-center justify-between gap-2 rounded-[4px] border border-line bg-ground px-2.5",
        "text-left text-13 text-fg transition-[border-color] duration-100",
        "hover:border-fg-dim data-[state=open]:border-accent",
        "disabled:pointer-events-none disabled:opacity-40",
        className,
      )}
      {...rest}
    >
      {children}
      <SelectPrimitive.Icon asChild>
        <ChevronDown className="size-4 shrink-0 text-fg-dim" />
      </SelectPrimitive.Icon>
    </SelectPrimitive.Trigger>
  );
}

export function SelectContent({
  className,
  children,
  position = "popper",
  ...rest
}: ComponentProps<typeof SelectPrimitive.Content>) {
  return (
    <SelectPrimitive.Portal>
      <SelectPrimitive.Content
        data-slot="select-content"
        position={position}
        sideOffset={4}
        className={cn(menuSurface, "max-h-[60vh]", className)}
        {...rest}
      >
        <SelectPrimitive.Viewport className="min-w-[var(--radix-select-trigger-width)]">
          {children}
        </SelectPrimitive.Viewport>
      </SelectPrimitive.Content>
    </SelectPrimitive.Portal>
  );
}

export function SelectItem({
  className,
  children,
  ...rest
}: ComponentProps<typeof SelectPrimitive.Item>) {
  return (
    <SelectPrimitive.Item
      data-slot="select-item"
      className={cn(menuItem, "pr-8 text-fg", className)}
      {...rest}
    >
      <SelectPrimitive.ItemText>{children}</SelectPrimitive.ItemText>
      <SelectPrimitive.ItemIndicator className="absolute right-2.5">
        <Check className="size-4 text-accent" />
      </SelectPrimitive.ItemIndicator>
    </SelectPrimitive.Item>
  );
}
