/** The two button shapes the chrome uses, so every surface agrees on padding,
 *  radius and what "pressed" looks like. No domain knowledge lives here. */

import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";

type Variant = "quiet" | "outline" | "accent" | "danger";

const VARIANTS: Record<Variant, string> = {
  quiet: "text-fg-mid hover:bg-hover hover:text-fg",
  outline: "border border-line text-fg-mid hover:bg-hover hover:text-fg",
  accent: "border border-accent-d bg-accent/15 text-accent hover:bg-accent/25",
  danger: "border border-danger/50 text-danger hover:bg-danger/15",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  /** Pressed-in look for a toggle that is currently on — the active tab, the
   *  open pane. Carries the accent, like every other "this one" signal. */
  active?: boolean;
  children?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "quiet", active, className, children, ...rest },
  ref,
) {
  return (
    <button
      ref={ref}
      type="button"
      className={`shrink-0 rounded-[4px] px-2 py-[3px] text-[12px] disabled:pointer-events-none disabled:opacity-40 ${
        active ? "bg-accent/15 text-accent" : VARIANTS[variant]
      } ${className ?? ""}`}
      {...rest}
    >
      {children}
    </button>
  );
});

/** Square, for a glyph. Always paired with a tooltip or an aria-label. */
export const IconButton = forwardRef<HTMLButtonElement, ButtonProps>(
  function IconButton({ variant = "quiet", active, className, children, ...rest }, ref) {
    return (
      <button
        ref={ref}
        type="button"
        className={`grid h-[26px] w-[26px] shrink-0 place-items-center rounded-[4px] text-[13px] leading-none disabled:pointer-events-none disabled:opacity-40 ${
          active ? "bg-accent/15 text-accent" : VARIANTS[variant]
        } ${className ?? ""}`}
        {...rest}
      >
        {children}
      </button>
    );
  },
);
