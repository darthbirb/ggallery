/** shadcn/ui's checkbox, restyled. A native `<input type=checkbox>` cannot be
 *  drawn to match a dark, dense chrome on Windows; this can. */

import * as CheckboxPrimitive from "@radix-ui/react-checkbox";
import { Check } from "lucide-react";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

export function Checkbox({
  className,
  ...rest
}: ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "peer size-4 shrink-0 rounded-[3px] border border-line bg-ground",
        "transition-[background-color,border-color] duration-100 hover:border-fg-dim",
        "data-[state=checked]:border-accent-d data-[state=checked]:bg-accent data-[state=checked]:text-ground",
        "disabled:pointer-events-none disabled:opacity-40",
        className,
      )}
      {...rest}
    >
      <CheckboxPrimitive.Indicator className="grid place-items-center text-current">
        <Check className="size-3.5" strokeWidth={3} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
