# Engineering notes

Platform behaviour learned the expensive way. Everything here cost real time to discover,
so read it before rediscovering it. Full evidence in [M0-RESULTS.md](M0-RESULTS.md).

---

## Tauri and Windows

**WebView2 writes outside the app directory by default.** Tauri points WebView2's user
data folder at `%LOCALAPPDATA%\<bundle-id>\` — the M0 spike accumulated 77MB there without
anyone asking it to. That silently breaks the "nothing outside the app directory and the
library root" rule that the whole portability promise rests on. Set the data directory
explicitly to a path inside the app directory before shipping anything, and verify it by
checking `%LOCALAPPDATA%` after a run.

**Heavy work in a `#[tauri::command]` blocks the native window message pump.** Not a
background thread — the whole window stops responding and Windows marks it "Not
Responding". Any command doing file IO, hashing, encoding or process orchestration must be
`async fn` with the work inside `tauri::async_runtime::spawn_blocking`. The M1 job queue
depends on this being right from the start; it is painful to retrofit.

**Never `assert!` or `debug_assert!` inside a command handler.** A failed `debug_assert!`
in M0 aborted the entire process with `STATUS_STACK_BUFFER_OVERRUN` rather than returning
an error, because the panic occurred where it could not unwind. Return `Result` and handle
the failure.

**`cargo build --release` is not a release build of the app.** Tauri only serves the built
frontend when compiled with the `custom-protocol` Cargo feature; without it the binary
tries to reach the dev server at `localhost:1420` regardless of optimisation level, and
opens to a blank "localhost refused to connect" window. Always build through the `tauri`
CLI.

**Debug builds are 6–40x slower on codec work.** WebP encode: 33ms debug, 5.2ms release.
Generating the 100k-item test library: 7.3 minutes debug, 3 minutes release. Do all
performance measurement on release builds — debug numbers are not merely pessimistic, they
are meaningless.

---

## Thumbnails

**WebP, via the `webp` crate (real libwebp bindings), lossy q78.** Measured against AVIF at
320px longest edge:

| | WebP | AVIF |
| --- | --- | --- |
| Encode | 5.2ms | 216ms |
| Decode | 0.5ms | never succeeded |
| Average size | 798 B | 706 B |

AVIF's 12% size advantage does not survive contact with a 41x encode penalty. The encoder
needs `nasm`-built assembly to be viable at all — without it `rav1e` falls back to a pure
Rust path taking over 9 seconds per thumbnail in debug. Worse, decode never worked: the
`image` crate 0.25 `avif` feature returned "format not supported" on 500/500 attempts while
happily driving the encoder. Treat AVIF as closed unless something changes upstream.

---

## Decoding

**`image::open` and `image::image_dimensions` choose the decoder from the file
extension, and extensions lie.** The first real library this was pointed at held six
JPEGs named `.PNG`, straight off a phone. Every one failed with `Format error decoding
Png: Invalid PNG signature` and indexed with no dimensions, while opening fine in every
other program on the machine.

Always go through `media::open_image_reader`, which is
`ImageReader::open(..).with_guessed_format()` — magic bytes first, extension only as the
fallback when sniffing is inconclusive. It costs one small read.

The failure is silent in two directions worth knowing about: probing swallows the error
(a file the app cannot parse is still a real file and must still appear in the grid), so
the only visible symptom was a missing thumbnail and a square tile. Anything that decodes
by extension will reproduce this, and a camera roll is where it shows up.

---

## Grid architecture (validated in M0)

The approach below hit every target with 2–100x margin at 100k items. M1 should reuse it
rather than re-derive it.

1. Precompute row breaks for the whole library **once**, in a web worker. Greedy justified
   layout: accumulate items until the row's width-at-target-height reaches container width,
   then scale that row to fill exactly.
2. Return typed arrays (`Float32Array` / `Uint32Array`) — row tops, row heights, per-row
   item start and count, per-item left and width.
3. On the main thread, binary-search the ascending `rowTops` array for the visible range,
   rAF-throttled. Render only rows in `[start - overscan, end + overscan]` as absolutely
   positioned tiles inside a container sized to the precomputed total height, so the
   scrollbar stays native and thumb-accurate.

Full 100k-item layout computes in 7–18ms, so incremental or partial relayout is
unnecessary at this scale. Keep it in the worker regardless — it decouples relayout from
image decode and React commits on the main thread, and costs nothing.

---

## The two defects M0 found — both fixed in M1, keep them fixed

Interaction-tagged re-testing (§1a of [M0-RESULTS.md](M0-RESULTS.md)) turned the earlier
vague "frame times are mostly fine" into two specific, located defects. Neither
invalidated the layout architecture above — first paint, relayout and scrubber-jump
latency all passed with wide margins. Both lived in the tile component and the scrubber,
and M1 shipped with both addressed.

They are recorded here because the fixes are easy to undo by accident. Anyone who
reintroduces a React component per tile, or drops the scrubber's per-frame coalescing,
reintroduces the measurements below.

### 1. Tile churn triggers GC pauses during fast scrolling — fling fails its target

| Interaction | p50 worst-frame | p95 | Max | Windows >32ms |
| --- | --- | --- | --- | --- |
| idle | 4.5ms | 4.7ms | 104.2ms | 0.8% |
| slow-scroll (1200px/s) | 4.4ms | 100.0ms | 100.0ms | 9% |
| **fling** | **16.7ms** | **104.2ms** | **104.2ms** | **33%** |
| slider-drag | 4.4ms | 16.6ms | 16.6ms | 0% |
| **scrubber-drag** | **33.5ms** | **108.3ms** | **108.3ms** | **58%** |

**Fling does not meet "no blank frame held over 100ms," and one pass is enough to trip
it** — the first burst of a run produced consecutive worst-frames of 88.8ms, 91.7ms,
100.0ms. This is not a sustained-flinging artifact.

The mechanism is visible directly in the logs and is not speculation: heap climbs from
~26MB to 40–60MB across several frames as rapidly-entering tiles decode, then a
100–105ms spike lands *exactly* as heap drops back to baseline. That is a major GC
collection, and it distinguishes cleanly from the idle stalls below, which spike with
heap *flat*.

**What this means for the production tile component.** Mounting and unmounting a React
component per tile — each creating a fresh `<img>` and decode object — generates enough
garbage per fast-scroll frame to force a collection. The fix is to stop allocating per
tile:

- **Recycle a fixed pool of tile DOM nodes.** Reposition and repopulate them as the
  visible range moves instead of mounting and unmounting components. This is the
  imperative-recycling escape hatch flagged earlier; it is no longer optional.
- **Reuse `<img>` elements**, setting `src` on pooled nodes rather than creating new ones.
- Keep per-tile decoration (favorite badge, selection ring, duration) as reused nodes
  toggled by class, not conditionally rendered children.

Shipped in M1: `grid/Tile.tsx` exports a `TilePool`, not a component. Nothing mounts or
unmounts while scrolling and React renders no tiles at all.

### 2. Scrubber drag repaints per jump

58% of scrubber-drag windows exceed 32ms, several past 100ms, reproduced across both
runs. Slider-drag over the same relayout path stays clean (max 16.6ms, zero windows over
32ms), which isolates the cause precisely: **the worker relayout is not the bottleneck —
the scrubber's per-jump repaint is.** Coalesce drag-driven jumps to one repaint per
animation frame. Discrete jumps need nothing; they already paint in 0.7ms.

### Not a defect: idle stalls

Isolated single-frame spikes of 33–125ms recur every 10–90 seconds at idle with heap flat
at ~26MB — not GC. Consistent with AMD Adrenalin's overlay hooking the swap-chain path;
WebView2 is a normal Chromium GPU client. The re-test deliberately left the overlay
running because that is the real end-user condition on an AMD machine, and equivalent
overlays ship for NVIDIA and Intel.

Treat this as an environmental characteristic of the target machine class, not something
to chase in the app. Note that the causal attribution is by elimination — the overlay was
never toggled off for a clean control — but the heap-flat signature rules out GC, which
is what matters for deciding it is not ours.
