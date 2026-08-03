/**
 * shadcn/ui's button, restyled to the app's dark, dense palette.
 *
 * The structure is shadcn's — `cva` variants, `asChild` through Radix's Slot,
 * `data-slot` for styling from a parent — and the values are ours. Locked
 * decision 25 is enforced here rather than remembered at each call site:
 *
 * - heights are `28 / 32 / 38` and nothing else, icon buttons never below
 *   `32×32` whatever the glyph;
 * - the glyph fills 55–60% of its button — 18px in a 32px button;
 * - **every variant has a background and a border at rest.** There is no
 *   ghost variant. A control that only appears on hover is invisible until
 *   you already know it is there, which is the defect this pass exists to
 *   fix.
 *
 * There are deliberately **no focus classes here**. The one `:focus-visible`
 * rule in `styles/index.css` covers every control in the app; adding
 * `outline-none` alongside a `focus-visible:outline-*` utility silently
 * cancels both, which is how keyboard focus went invisible once already.
 */

import { Slot } from "@radix-ui/react-slot";
import { cva, type VariantProps } from "class-variance-authority";
import { forwardRef, type ButtonHTMLAttributes } from "react";

import { cn } from "../../lib/utils";

const buttonVariants = cva(
  "inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap rounded-[4px] border font-medium " +
    "transition-[background-color,border-color,color] duration-100 " +
    "disabled:pointer-events-none disabled:opacity-40 " +
    "[&_svg]:pointer-events-none [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "border-line bg-raised text-fg-mid hover:border-fg-dim hover:bg-hover hover:text-fg",
        accent:
          "border-accent-d bg-accent/15 text-accent hover:bg-accent/25 hover:border-accent",
        danger:
          "border-danger/45 bg-danger/12 text-danger hover:border-danger/70 hover:bg-danger/22",
        good: "border-good/45 bg-good/12 text-good hover:border-good/70 hover:bg-good/22",
      },
      size: {
        sm: "h-7 px-2.5 text-[13px] [&_svg]:size-4",
        default: "h-8 px-3 text-[14px] [&_svg]:size-[18px]",
        lg: "h-[38px] px-4 text-[14px] [&_svg]:size-[22px]",
        icon: "size-8 p-0 [&_svg]:size-[18px]",
        "icon-lg": "size-[38px] p-0 [&_svg]:size-[22px]",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  },
);

export interface ButtonProps
  extends ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
  /** Pressed-in look for a toggle that is currently on — the active tab, the
   *  open pane. Carries the accent, like every other "this one" signal. */
  active?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { className, variant, size, active, asChild, type, ...rest },
  ref,
) {
  const Comp = asChild ? Slot : "button";
  return (
    <Comp
      ref={ref}
      data-slot="button"
      data-active={active ? "" : undefined}
      type={asChild ? undefined : (type ?? "button")}
      className={cn(
        buttonVariants({ variant: active ? "accent" : variant, size }),
        className,
      )}
      {...rest}
    />
  );
});

/**
 * Square, for a glyph, and never smaller than `32×32`. Always paired with a
 * tooltip or an `aria-label` — the icon is the whole label.
 */
export const IconButton = forwardRef<HTMLButtonElement, ButtonProps>(
  function IconButton({ size = "icon", ...rest }, ref) {
    return <Button ref={ref} size={size} {...rest} />;
  },
);

export { buttonVariants };
