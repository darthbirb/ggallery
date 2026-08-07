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
        "peer size-[18px] shrink-0 rounded-[4px] border border-line bg-ground",
        "transition-[background-color,border-color] duration-100 hover:border-fg-dim",
        // The tick is `--sunk`, not `--ground`: it is punched out of the
        // accent fill and wants the darkest grey in the set behind it.
        "data-[state=checked]:border-accent data-[state=checked]:bg-accent data-[state=checked]:text-sunk",
        "disabled:pointer-events-none disabled:opacity-40",
        className,
      )}
      {...rest}
    >
      <CheckboxPrimitive.Indicator className="grid place-items-center text-current">
        <Check className="size-[13px]" strokeWidth={3} />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
