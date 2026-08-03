/** shadcn/ui's separator, restyled — the hairlines between groups of controls
 *  in the header, the selection bar and the folded navigation strip. */

import * as SeparatorPrimitive from "@radix-ui/react-separator";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

export function Separator({
  className,
  orientation = "vertical",
  decorative = true,
  ...rest
}: ComponentProps<typeof SeparatorPrimitive.Root>) {
  return (
    <SeparatorPrimitive.Root
      data-slot="separator"
      orientation={orientation}
      decorative={decorative}
      className={cn(
        "shrink-0 bg-line-soft",
        orientation === "vertical" ? "h-5 w-px" : "h-px w-full",
        className,
      )}
      {...rest}
    />
  );
}
