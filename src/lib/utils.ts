import { clsx, type ClassValue } from "clsx";
import { extendTailwindMerge } from "tailwind-merge";

/**
 * M2.8b's type scale is numeric — `text-10` … `text-28`, from the `--text-*`
 * tokens in `styles/index.css` — and tailwind-merge has to be told about it.
 *
 * Out of the box it reads `text-13` as a **colour**, because its font-size
 * validator only recognises t-shirt names and arbitrary lengths. That made
 * `cn("text-13", "text-fg-mid")` collapse to `text-fg-mid` and silently drop
 * the size — and since almost every call site pairs a size with a colour, it
 * would have dropped the size almost everywhere, in a way that looks like the
 * token layer simply not working rather than like a bug.
 *
 * Registering the scale in the `font-size` group fixes it at the root. Keep
 * this list in step with `@theme`: a size that is in one and not the other is
 * either a class Tailwind never generates or a merge that misbehaves.
 */
const twMerge = extendTailwindMerge({
  extend: {
    classGroups: {
      "font-size": [
        { text: ["10", "11", "12", "13", "14", "15", "16", "26", "28"] },
      ],
    },
  },
});

/**
 * shadcn/ui's class merger, and the reason its components can be restyled at
 * the call site: `cn("px-2", props.className)` lets a later utility win over
 * an earlier one instead of both landing in the class list and the cascade
 * deciding by declaration order.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
