/**
 * One item, rendered to fill whatever box it is given.
 *
 * This is the unit Preview mode tiles: one of these per pane, so two of them
 * side by side is compression review (M6) and duplicate comparison (M7), and
 * twelve is multi-view (M10). It therefore takes everything it needs as props
 * and owns no knowledge of how many of it exist.
 *
 * **Images have no zoom UI at fit** — no fit button, no 1:1 button, no
 * percentage readout, nothing on screen (docs/DESIGN.md §2). Scroll and drag
 * are the whole interaction there. Once zoom leaves fit, a single small
 * percentage readout appears in a corner and doubles as the discoverable
 * form of the double-click-to-fit gesture — the rule this replaced banned a
 * *permanent* strip of chrome competing with the photograph; a control
 * that is absent until it is relevant does not compete with anything.
 *
 * Volume is deliberately module-level: docs/DESIGN.md §2 asks for volume that
 * persists between items, and an item change unmounts the `<video>`.
 */

import {
  ChevronFirst,
  ChevronLast,
  FileQuestion,
  Pause,
  Play,
  Volume2,
  VolumeX,
  X,
} from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";

import { DropdownMenu, MenuItem, MenuLabel } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { Button, IconButton } from "../../components/ui/button";
import { Slider } from "../../components/ui/slider";
import { formatDuration } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { ItemDetail } from "../../lib/types";

let sharedVolume = 1;
let sharedMuted = false;

const SPEEDS = [0.25, 0.5, 1, 1.5, 2];
/** No frame rate is recorded, so a step is a nominal frame at 30fps. */
const FRAME_STEP = 1 / 30;

export function ItemView({ item }: { item: ItemDetail }) {
  const source = ipc.assetPath(item.path);

  if (item.kind === "video") {
    return <VideoView key={item.id} item={item} source={source} />;
  }
  if (item.kind === "image") {
    return <ImageView key={item.id} item={item} source={source} />;
  }
  return (
    <div className="flex h-full flex-col items-center justify-center gap-2 px-4 text-fg-dim">
      <FileQuestion className="size-7" />
      <p className="max-w-[34ch] text-center">
        {item.origName ?? item.diskName} is not something this app displays.
        Open it with the default application instead.
      </p>
    </div>
  );
}

// --- images ----------------------------------------------------------------

function ImageView({ item, source }: { item: ItemDetail; source: string }) {
  const [zoom, setZoom] = useState<number | null>(null); // null = fit
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const dragging = useRef<{ x: number; y: number } | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);

  // Double-click returns to fit. Not a control — a gesture, alongside the
  // other two — so it adds nothing to the surface.
  const reset = useCallback(() => {
    setZoom(null);
    setPan({ x: 0, y: 0 });
  }, []);

  // The scale "fit" is actually rendering at, so the first wheel step out of
  // it starts from what is already on screen instead of from 100% of the
  // image's native pixels. Without this, an image much larger or smaller
  // than the pane jumps the instant zoom leaves fit — a big native photo in
  // a small pane, say, snapping from a fifth of its size to full size in one
  // scroll tick. `<= 1` matches the fit rendering itself, which shrinks a
  // large image to the box but never stretches a small one past its own
  // size (`max-h-full max-w-full`, no `width`/`height` forcing it wider).
  const fitScale = () => {
    const box = containerRef.current;
    if (!box || !item.width || !item.height) return 1;
    return Math.min(1, box.clientWidth / item.width, box.clientHeight / item.height);
  };

  return (
    <div
      ref={containerRef}
      className="relative h-full min-h-0 overflow-hidden"
      onWheel={(event) => {
        // Scroll to zoom, drag to pan — DESIGN.md §2 "Preview mode". A point
        // at image-space p lands at screen offset p·zoom + pan (offset from
        // the container's centre), so p = (cursor - pan) / zoom for whatever
        // sits under the pointer right now. Solving the same equation for
        // the new zoom with p held fixed gives the pan that keeps that exact
        // point under the cursor: pan' = cursor - p·zoom'. Substituting
        // cursor = 0 (the container's centre) is what the old fixed-centre
        // version amounted to; the cursor's own offset is what the wheel has
        // already told us to look at instead.
        const box = containerRef.current;
        const rect = box?.getBoundingClientRect();
        const cursor = rect
          ? {
              x: event.clientX - rect.left - rect.width / 2,
              y: event.clientY - rect.top - rect.height / 2,
            }
          : { x: 0, y: 0 };
        const current = zoom ?? fitScale();
        const next = Math.min(
          Math.max(current * (event.deltaY < 0 ? 1.12 : 0.89), 0.1),
          12,
        );
        const imagePoint = { x: (cursor.x - pan.x) / current, y: (cursor.y - pan.y) / current };
        setPan({ x: cursor.x - imagePoint.x * next, y: cursor.y - imagePoint.y * next });
        setZoom(next);
      }}
      onMouseDown={(event) => {
        if (event.button !== 0) return;
        dragging.current = { x: event.clientX - pan.x, y: event.clientY - pan.y };
      }}
      onMouseMove={(event) => {
        if (!dragging.current) return;
        setPan({
          x: event.clientX - dragging.current.x,
          y: event.clientY - dragging.current.y,
        });
      }}
      onMouseUp={() => {
        dragging.current = null;
      }}
      onMouseLeave={() => {
        dragging.current = null;
      }}
      onDoubleClick={reset}
    >
      <img
        src={source}
        alt={item.origName ?? item.diskName}
        draggable={false}
        style={
          zoom === null
            ? undefined
            : {
                transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
                maxWidth: "none",
                maxHeight: "none",
                width: item.width ?? undefined,
                height: item.height ?? undefined,
              }
        }
        className={
          zoom === null
            ? "absolute inset-0 m-auto max-h-full max-w-full object-contain"
            : "absolute left-1/2 top-1/2 origin-center -translate-x-1/2 -translate-y-1/2"
        }
      />

      {/* Absent at fit — DESIGN.md §2 "Preview mode". Once zoom leaves fit
          this is the only zoom UI there is: a readout that doubles as the
          discoverable form of the double-click-to-fit gesture. Bright, not
          dimmed-until-hover — dimming a control that only appears when it is
          already relevant reads as "ignore me", the opposite of the point.
          The trailing `X` is what says "click removes this" without
          requiring the hover state to find out. */}
      {zoom !== null && (
        <button
          type="button"
          aria-label={`Zoom ${Math.round(zoom * 100)}% — click to fit`}
          onClick={reset}
          className="absolute bottom-2 right-2 inline-flex items-center gap-1 rounded-full border border-line-soft bg-ground/90 py-0.5 pl-2 pr-1.5 font-mono text-[12px] tabular-nums text-fg hover:border-fg-dim hover:bg-ground"
        >
          {Math.round(zoom * 100)}%
          <X className="size-3 text-fg-dim" />
        </button>
      )}
    </div>
  );
}

// --- video -----------------------------------------------------------------

function VideoView({ item, source }: { item: ItemDetail; source: string }) {
  const video = useRef<HTMLVideoElement>(null);
  const [playing, setPlaying] = useState(false);
  const [position, setPosition] = useState(0);
  const [duration, setDuration] = useState((item.durationMs ?? 0) / 1000);
  const [speed, setSpeed] = useState(1);
  const [muted, setMuted] = useState(sharedMuted);

  useEffect(() => {
    const element = video.current;
    if (!element) return;
    element.volume = sharedVolume;
    element.muted = sharedMuted;
  }, []);

  const seek = (seconds: number) => {
    const element = video.current;
    if (!element) return;
    element.currentTime = Math.min(
      Math.max(seconds, 0),
      duration || element.duration || 0,
    );
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="relative min-h-0 flex-1">
        <video
          ref={video}
          src={source}
          // Loop is on by default — DESIGN.md §2 "Preview mode".
          loop
          autoPlay
          playsInline
          onPlay={() => setPlaying(true)}
          onPause={() => setPlaying(false)}
          onTimeUpdate={(event) => setPosition(event.currentTarget.currentTime)}
          onLoadedMetadata={(event) => setDuration(event.currentTarget.duration)}
          onVolumeChange={(event) => {
            sharedVolume = event.currentTarget.volume;
            sharedMuted = event.currentTarget.muted;
            setMuted(event.currentTarget.muted);
          }}
          className="absolute inset-0 h-full w-full object-contain"
        />
      </div>

      {/* Transport, not chrome: a video cannot be played without these, which
          is what separates them from the zoom toolbar images used to carry. */}
      <div className="flex shrink-0 items-center gap-1.5 border-t border-line bg-panel px-2 py-1.5">
        <IconButton
          aria-label={playing ? "Pause" : "Play"}
          variant="accent"
          onClick={() => {
            const element = video.current;
            if (!element) return;
            if (element.paused) void element.play();
            else element.pause();
          }}
        >
          {playing ? <Pause /> : <Play />}
        </IconButton>

        <Tooltip label="Back one frame" side="top">
          <IconButton
            aria-label="Back one frame"
            onClick={() => seek(position - FRAME_STEP)}
          >
            <ChevronFirst />
          </IconButton>
        </Tooltip>
        <Tooltip label="Forward one frame" side="top">
          <IconButton
            aria-label="Forward one frame"
            onClick={() => seek(position + FRAME_STEP)}
          >
            <ChevronLast />
          </IconButton>
        </Tooltip>

        <Slider
          aria-label="Position"
          className="mx-1 min-w-0 flex-1"
          min={0}
          max={Math.max(duration, 0.001)}
          step={0.01}
          value={[position]}
          onValueChange={([next]) => seek(next)}
        />

        <span className="shrink-0 font-mono tabular-nums text-fg-dim">
          {formatDuration(position * 1000)} / {formatDuration(duration * 1000)}
        </span>

        <DropdownMenu
          align="end"
          trigger={
            <Button size="sm" aria-label="Playback speed">
              {speed}×
            </Button>
          }
        >
          <MenuLabel>Speed</MenuLabel>
          {SPEEDS.map((option) => (
            <MenuItem
              key={option}
              onSelect={() => {
                setSpeed(option);
                if (video.current) video.current.playbackRate = option;
              }}
            >
              {option === speed ? `● ${option}×` : `${option}×`}
            </MenuItem>
          ))}
        </DropdownMenu>

        <IconButton
          aria-label={muted ? "Unmute" : "Mute"}
          onClick={() => {
            const element = video.current;
            if (!element) return;
            element.muted = !element.muted;
          }}
        >
          {muted ? <VolumeX /> : <Volume2 />}
        </IconButton>
      </div>
    </div>
  );
}
