/**
 * shadcn/ui's input and textarea, restyled.
 *
 * Text fields are the one control where focus is drawn on click as well as by
 * keyboard — the caret has to have somewhere to be — so these take an accent
 * border on plain `:focus`, on top of the app-wide `:focus-visible` ring. That
 * is not the exception decision 26 forbids: the rule is about selection
 * competing with a focus ring on the same tile, not about a field showing
 * where you are typing.
 */

import { forwardRef, type InputHTMLAttributes, type TextareaHTMLAttributes } from "react";

import { cn } from "../../lib/utils";

const FIELD =
  "w-full rounded-[4px] border border-line bg-ground text-[14px] text-fg " +
  "placeholder:text-fg-dim " +
  "transition-[border-color] duration-100 focus:border-accent-d " +
  "disabled:pointer-events-none disabled:opacity-40";

export const Input = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(
  function Input({ className, type, ...rest }, ref) {
    return (
      <input
        ref={ref}
        data-slot="input"
        type={type ?? "text"}
        className={cn(FIELD, "h-8 px-2", className)}
        {...rest}
      />
    );
  },
);

export const Textarea = forwardRef<
  HTMLTextAreaElement,
  TextareaHTMLAttributes<HTMLTextAreaElement>
>(function Textarea({ className, ...rest }, ref) {
  return (
    <textarea
      ref={ref}
      data-slot="textarea"
      className={cn(FIELD, "min-h-[56px] resize-none px-2 py-1.5", className)}
      {...rest}
    />
  );
});

/**
 * The inline "add a tag" field: a dashed pill that grows when you type in it.
 * Same focus treatment, different shape, so tags read as tags rather than as
 * another form field.
 */
export const PillInput = forwardRef<
  HTMLInputElement,
  InputHTMLAttributes<HTMLInputElement>
>(function PillInput({ className, ...rest }, ref) {
  return (
    <input
      ref={ref}
      data-slot="pill-input"
      type="text"
      className={cn(
        "h-7 w-24 rounded-full border border-dashed border-line bg-transparent px-2.5 text-[13px] text-fg-mid",
        "placeholder:text-fg-dim transition-[width,border-color] duration-100",
        "focus:w-40 focus:border-accent-d focus:text-fg",
        className,
      )}
      {...rest}
    />
  );
});

export { FIELD as fieldClassName };
