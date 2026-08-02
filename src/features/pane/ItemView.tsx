/**
 * One item, rendered to fill whatever box it is given.
 *
 * This is the unit Preview mode tiles: one of these per pane, so two of them
 * side by side is compression review (M6) and duplicate comparison (M7), and
 * twelve is multi-view (M10). It therefore takes everything it needs as props
 * and owns no knowledge of how many of it exist.
 *
 * Volume is deliberately module-level: docs/DESIGN.md §2 asks for volume that
 * persists between items, and an item change unmounts the `<video>`.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import { Button, IconButton } from "../../components/Button";
import { DropdownMenu, MenuItem, MenuLabel } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
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
    <div className="flex h-full flex-col items-center justify-center gap-2 text-fg-dim">
      <span className="text-[22px]">◫</span>
      <p className="max-w-[34ch] text-center text-[12px]">
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

  const reset = useCallback(() => {
    setZoom(null);
    setPan({ x: 0, y: 0 });
  }, []);

  return (
    <div className="relative flex h-full min-h-0 flex-col">
      <div
        className="relative min-h-0 flex-1 overflow-hidden"
        onWheel={(event) => {
          // Scroll to zoom, drag to pan — DESIGN.md §2 "Preview mode".
          const current = zoom ?? 1;
          const next = Math.min(Math.max(current * (event.deltaY < 0 ? 1.12 : 0.89), 0.1), 12);
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
      </div>

      <div className="flex shrink-0 items-center gap-1 border-t border-line-soft px-2 py-1">
        <Button variant="quiet" active={zoom === null} onClick={reset}>
          Fit
        </Button>
        <Button
          variant="quiet"
          active={zoom === 1}
          onClick={() => {
            setZoom(1);
            setPan({ x: 0, y: 0 });
          }}
        >
          1:1
        </Button>
        <span className="ml-auto font-mono text-[11px] tabular-nums text-fg-dim">
          {zoom === null ? "fit" : `${Math.round(zoom * 100)}%`}
        </span>
      </div>
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
    element.currentTime = Math.min(Math.max(seconds, 0), duration || element.duration || 0);
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

      <div className="flex shrink-0 items-center gap-1 border-t border-line-soft px-2 py-1">
        <IconButton
          aria-label={playing ? "Pause" : "Play"}
          onClick={() => {
            const element = video.current;
            if (!element) return;
            if (element.paused) void element.play();
            else element.pause();
          }}
        >
          {playing ? "❚❚" : "▶"}
        </IconButton>

        <Tooltip label="Back one frame" side="top">
          <IconButton aria-label="Back one frame" onClick={() => seek(position - FRAME_STEP)}>
            ⟨
          </IconButton>
        </Tooltip>
        <Tooltip label="Forward one frame" side="top">
          <IconButton
            aria-label="Forward one frame"
            onClick={() => seek(position + FRAME_STEP)}
          >
            ⟩
          </IconButton>
        </Tooltip>

        <input
          type="range"
          aria-label="Position"
          min={0}
          max={Math.max(duration, 0.001)}
          step={0.01}
          value={position}
          onChange={(event) => seek(Number(event.target.value))}
          className="mx-1 min-w-0 flex-1 accent-[var(--color-accent)]"
        />

        <span className="shrink-0 font-mono text-[11px] tabular-nums text-fg-dim">
          {formatDuration(position * 1000)} / {formatDuration(duration * 1000)}
        </span>

        <DropdownMenu
          align="end"
          trigger={
            <Button variant="quiet" aria-label="Playback speed">
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
          {muted ? "🔇" : "🔊"}
        </IconButton>
      </div>
    </div>
  );
}
