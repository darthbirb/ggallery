/** shadcn/ui's slider, restyled — the tile-size control, the video position
 *  bar, and the panel widths in Settings. Pointer capture, keyboard stepping
 *  and RTL are Radix's; the look is ours. */

import * as SliderPrimitive from "@radix-ui/react-slider";
import type { ComponentProps } from "react";

import { cn } from "../../lib/utils";

export function Slider({
  className,
  ...rest
}: ComponentProps<typeof SliderPrimitive.Root>) {
  return (
    <SliderPrimitive.Root
      data-slot="slider"
      className={cn(
        "relative flex h-8 w-full cursor-pointer touch-none select-none items-center",
        className,
      )}
      {...rest}
    >
      <SliderPrimitive.Track
        data-slot="slider-track"
        className="relative h-1 w-full grow overflow-hidden rounded-full bg-raised"
      >
        <SliderPrimitive.Range
          data-slot="slider-range"
          className="absolute h-full bg-accent"
        />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        data-slot="slider-thumb"
        // 14px: small enough to stay dense, big enough to grab without
        // aiming. The drawing makes the thumb `--fg` inside a 2px ring of the
        // panel behind it, not accent-on-accent — so the thumb stays legible
        // where it overlaps its own filled range, which an accent thumb on an
        // accent range does not.
        className={cn(
          "block size-[14px] rounded-full border-2 border-panel bg-fg",
          "transition-[border-color] duration-100",
          "focus-visible:outline-offset-2",
        )}
      />
    </SliderPrimitive.Root>
  );
}
