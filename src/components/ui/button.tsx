/**
 * shadcn/ui's button, restyled to the app's dark, dense palette.
 *
 * The structure is shadcn's — `cva` variants, `asChild` through Radix's Slot,
 * `data-slot` for styling from a parent — and the values are ours. Locked
 * decision 25 is enforced here rather than remembered at each call site, and
 * M2.8b replaced its numbers with the ones inventoried from the drawing:
 *
 * - heights are `26 / 28 / 32 / 38` for a control with a surface, and
 *   `16 / 18 / 20` for a sub-control that sits inside one;
 * - the glyph is smaller beside a label than alone — `16px` in a labelled
 *   32px button, `18px` in a square one — which is what keeps a label and a
 *   glyph reading as one group rather than as a glyph with text after it;
 * - **every variant with a surface has a background and a border at rest.**
 *   A control that only appears on hover is invisible until you already know
 *   it is there.
 *
 * **`subtle` is the one variant with no surface, and it is not a ghost
 * button.** It is the drawing's sub-control family — a chevron, a row's `+`
 * and `⋯`, a chip's remove `×` — things that live *inside* another control
 * and would read as a second button if they had their own frame. It hovers to
 * a translucent white overlay rather than to `--color-hover`, because the
 * drawing's own note says why: *"The old grey hover on the chevron vanished
 * on the accented selected row. A translucent white overlay reads on every
 * row state, so one rule covers all of them."*
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
        // 600, not 500 — the drawing's Components screen gives the accent
        // button the one weight step up in the family.
        accent:
          "border-accent-d bg-accent-t font-semibold text-accent hover:border-accent hover:bg-accent-t2",
        danger:
          "border-danger/45 bg-danger/12 text-danger hover:border-danger/70 hover:bg-danger/22",
        good: "border-good/45 bg-good/12 text-good hover:border-good/70 hover:bg-good/22",
        // No surface, no border, and a translucent overlay on hover so it
        // reads the same on a plain row and on an accent-tinted selected one.
        // 3px, not 4: a sub-control's corner sits inside another control's,
        // and matching it would make the two read as one nested frame.
        subtle:
          "rounded-[3px] border-transparent bg-transparent text-fg-dim hover:bg-white/10 hover:text-fg",
      },
      size: {
        // A chip-height control: the dashed ＋ add buttons, the status chip.
        xs: "h-[26px] px-2.5 text-12 [&_svg]:size-3",
        sm: "h-7 px-[11px] text-13 [&_svg]:size-[15px]",
        default: "h-8 px-3 text-13 [&_svg]:size-4",
        lg: "h-[38px] px-4 text-14 [&_svg]:size-[17px]",
        // Square, with a surface. The glyph goes up when there is no label
        // beside it to share the centre with.
        icon: "size-8 p-0 [&_svg]:size-[18px]",
        "icon-lg": "size-[38px] p-0 [&_svg]:size-[22px]",
        // Square sub-controls, for `variant="subtle"`. Named for what they
        // sit inside, because that is what fixes the size: a field's clear
        // ×, a chip's remove ×, a row's chevron, a segment or a toast's
        // dismiss.
        "sub-xs": "size-4 p-0 [&_svg]:size-3",
        "sub-sm": "size-[18px] p-0 [&_svg]:size-[11px]",
        sub: "size-5 p-0 [&_svg]:size-[15px]",
        "sub-lg": "size-6 p-0 [&_svg]:size-[14px]",
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
