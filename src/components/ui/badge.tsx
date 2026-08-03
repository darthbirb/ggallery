/** shadcn/ui's badge, restyled — counts on navigation rows and queue badges on
 *  the folded strip. Tabular numerals so a count changing does not shuffle the
 *  row beside it. */

import { cva, type VariantProps } from "class-variance-authority";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

const badgeVariants = cva(
  "inline-flex shrink-0 items-center justify-center rounded-full border font-mono text-[12px] tabular-nums leading-none",
  {
    variants: {
      variant: {
        default: "border-line-soft bg-raised px-1.5 py-0.5 text-fg-mid",
        accent: "border-accent-d bg-accent/15 px-1.5 py-0.5 text-accent",
        danger: "border-danger/45 bg-danger/12 px-1.5 py-0.5 text-danger",
        bare: "border-transparent px-0 py-0 text-fg-dim",
      },
    },
    defaultVariants: { variant: "default" },
  },
);

export function Badge({
  className,
  variant,
  ...rest
}: ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return (
    <span
      data-slot="badge"
      className={cn(badgeVariants({ variant }), className)}
      {...rest}
    />
  );
}
