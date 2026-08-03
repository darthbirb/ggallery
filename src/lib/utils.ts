import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/**
 * shadcn/ui's class merger, and the reason its components can be restyled at
 * the call site: `cn("px-2", props.className)` lets a later utility win over
 * an earlier one instead of both landing in the class list and the cascade
 * deciding by declaration order.
 */
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
