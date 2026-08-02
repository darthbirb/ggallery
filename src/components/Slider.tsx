/** The tile-size control and the video scrubber, over Radix. Pointer capture,
 *  keyboard stepping and RTL are the primitive's; the look is ours. */

import * as RadixSlider from "@radix-ui/react-slider";

export function Slider({
  value,
  min,
  max,
  step = 1,
  onChange,
  onCommit,
  width,
  className,
  label,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  onChange: (value: number) => void;
  onCommit?: (value: number) => void;
  width?: number;
  className?: string;
  label: string;
}) {
  return (
    <RadixSlider.Root
      aria-label={label}
      value={[value]}
      min={min}
      max={max}
      step={step}
      onValueChange={([next]) => onChange(next)}
      onValueCommit={([next]) => onCommit?.(next)}
      style={width ? { width } : undefined}
      className={`relative flex h-4 touch-none select-none items-center ${className ?? ""}`}
    >
      <RadixSlider.Track className="relative h-[3px] w-full grow rounded-full bg-line">
        <RadixSlider.Range className="absolute h-full rounded-full bg-accent" />
      </RadixSlider.Track>
      <RadixSlider.Thumb className="block h-3 w-3 rounded-full border border-accent-d bg-accent outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent" />
    </RadixSlider.Root>
  );
}
