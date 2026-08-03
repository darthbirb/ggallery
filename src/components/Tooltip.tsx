/** The app's tooltip: a label and a side, over shadcn/ui's primitive. Icon-only
 *  controls carry one; the folded navigation panel is entirely icon-only, so
 *  this is what keeps it readable. */

import type { ReactNode } from "react";

import {
  TooltipContent,
  TooltipProvider,
  TooltipRoot,
  TooltipTrigger,
} from "./ui/tooltip";

export { TooltipProvider };

export function Tooltip({
  label,
  side = "right",
  children,
}: {
  label: ReactNode;
  side?: "top" | "right" | "bottom" | "left";
  children: ReactNode;
}) {
  return (
    <TooltipRoot>
      <TooltipTrigger asChild>{children}</TooltipTrigger>
      <TooltipContent side={side}>{label}</TooltipContent>
    </TooltipRoot>
  );
}
