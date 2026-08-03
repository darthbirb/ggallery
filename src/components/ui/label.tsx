/** shadcn/ui's label, restyled. Radix's version forwards clicks to the control
 *  it names and disables text selection on double-click, which a bare `<label>`
 *  does not. */

import * as LabelPrimitive from "@radix-ui/react-label";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

export function Label({
  className,
  ...rest
}: ComponentProps<typeof LabelPrimitive.Root>) {
  return (
    <LabelPrimitive.Root
      data-slot="label"
      className={cn(
        "flex select-none items-center gap-1.5 text-[13px] text-fg-mid",
        "peer-disabled:pointer-events-none peer-disabled:opacity-40",
        className,
      )}
      {...rest}
    />
  );
}
