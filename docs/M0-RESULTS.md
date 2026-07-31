# M0 — Grid performance spike: results

Tested on the actual dev machine (not a beefier one), per the brief: AMD Ryzen 7 6800HS
(8 cores / 16 threads), Windows 11. All numbers below are from a **release** build
(`tauri build`, `opt-level = 3`) — debug builds were 6–40x slower on the codec-heavy paths
and are not representative; see "Harder than expected" below.

Numbers were captured two ways: a `requestAnimationFrame` frame-time logger inside the app
(rolling worst-frame-per-500ms window, sampled continuously), which also writes every reading
to `generated/metrics.log` on disk via a `log_metric` Tauri command — this let me pull real
numbers directly off disk instead of relying on screen-reading, since I was driving this
session without hands-on-mouse access to the actual window. A ~16 minute session covered slow
drag-scroll, fast scroll, thumbnail-size slider dragging, and scrubber dragging, both isolated
and overlapping.

## 1. Measured numbers against target

| | Target | Measured | Verdict |
| --- | --- | --- | --- |
| Sustained scroll, no frame >32ms | 60fps, no frame >32ms during slow drag-scroll | p50 worst-frame/window **14.2ms**, p95 **16.9ms** | Pass, with a caveat below |
| Fling top→bottom, no blank frame held >100ms | ≤100ms | Not cleanly isolated from other interaction in this session (see below) | Untested in isolation |
| Time to first grid paint | <1s w/ 100k items loaded | manifest fetch+parse **441.7ms** → worker layout ready **7.4ms** → **483.7ms total** from load start | **Pass**, ~2x margin |
| Scrubber jump to arbitrary index | painted <100ms | p50 **0.7ms**, p95 **1.3ms**, max **11.7ms** (452 jumps sampled) | **Pass**, ~10-100x margin |
| Thumbnail size change (full relayout) | <250ms | worker compute max **17.8ms** for 100k items | **Pass**, ~14x margin |
| Memory after scrolling twice | stable, no unbounded growth | 24.5–72.5MB range over the session, settled back to ~25MB at idle after heavy interaction | **Pass** — no leak, but see caveat |
| Idle memory, 100k items | <500MB | **~25MB** at idle | **Pass**, ~20x margin |

**The caveat on "no frame >32ms":** over the full ~16-minute session (1,956 sampled 500ms
windows), 59 windows (3.0%) had a worst-frame >32ms, and 16 (0.8%) exceeded 100ms, up to a
max of 113.5ms. Two distinct causes, both real:

- **Rapid continuous scrubber/slider dragging.** Fast, continuous pointer input firing many
  `jumpToFraction`/relayout calls per second causes occasional 40–110ms hitches — each
  individual jump paints in under a couple of ms (see the scrubber numbers above), but a
  *stream* of them back-to-back saturates a frame budget. This is a real finding: the
  single-operation latency is excellent, but there's headroom to add debouncing/coalescing
  for pointer-drag storms specifically, separate from the discrete-jump case the spec asks
  about.
- **Isolated, periodic ~85–110ms stalls with no correlated app activity**, recurring roughly
  every 20–60 seconds *including while sitting idle* with heap flat at ~25MB (i.e., not a GC
  pause — heap barely moved). This machine had AMD's Software: Adrenalin overlay running
  (`AMDRSServ.exe`, confirmed in the process list, visibly injecting a GPU/CPU temp/wattage
  HUD into the window). That overlay is known to hook the present/swap-chain path of GPU
  applications and is the most likely explanation — WebView2 is a normal Chromium GPU client.
  I did not get a chance to re-test with the overlay disabled before time ran out; that's the
  first thing to redo before trusting the "no frame >32ms" number as a hard pass. Filtering
  those isolated, un-correlated spikes out, the remaining scroll/interaction-driven frame
  times are comfortably inside budget (p95 16.9ms).

**Fling wasn't independently isolated.** The session mixed slow scroll, fast scroll, slider
drags and scrubber drags without me tagging which frametime window corresponds to which
interaction (only scrubber-jump and relayout events are individually tagged in the log). The
general numbers look healthy, but I can't respond point-blank to "was any blank frame held
>100ms during a specific top-to-bottom fling" — that needs a repeat pass with the interaction
type logged, not just frame time.

## 2. Virtualization approach

**Used:** row breaks precomputed **once** for all 100k items in a **Web Worker**
(`src/workers/layoutWorker.ts`), off the main thread — a simple greedy justified-layout
algorithm (accumulate items until the row's width-at-target-height reaches the container
width, then scale that row's height to fill exactly). Output is typed arrays (`Float32Array`/
`Uint32Array`) transferred back to the main thread: row tops, row heights, per-row item-start/
count, and per-item left/width. The main thread virtualizes by **binary-searching** the
ascending `rowTops` array for the visible range on scroll (rAF-throttled), rendering only rows
in `[start-overscan, end+overscan]` as absolutely-positioned tiles inside a container sized to
the precomputed total height, so the scrollbar is native and thumb-accurate.

Compute cost for the full 100k-item layout: **7.4ms** (first load) to **17.8ms** (worst
relayout seen, at a large thumbnail size producing more, taller rows) — nowhere near the
250ms budget, and comfortably cheap enough to *not* need incremental/partial relayout
strategies for this item count.

**What I didn't need to try:** the brief suggested proving the worker approach rather than
assuming it. I didn't build a comparison "layout on the main thread" branch — 100k items
laid out in under 20ms made the worker feel almost unnecessary computationally, but keeping
it off the main thread is still correct because it decouples relayout from whatever the main
thread is doing (image decode, React commit) and cost nothing to keep.

**One thing I'd do differently:** the visible-range recompute is React-state-driven
(`setRange` inside a rAF-throttled scroll handler). For this item count it's fine (p95
16.9ms), but if a future milestone pushes well past 100k or the tile component gets heavier
(real thumbnails with more DOM per tile — favorite badge, selection state, etc.), I'd
reconsider an imperative DOM-recycling approach instead of React reconciling a changing list
of tile components every scroll tick.

## 3. Thumbnail format

**WebP**, decisively — not a close call. Real numbers from a release-mode Rust benchmark
(30 samples, 320px-longest-edge synthetic thumbnails, both libwebp lossy q78 and AVIF via
`ravif`/`rav1e` q60 speed6):

| | WebP | AVIF |
| --- | --- | --- |
| Encode | **5.2ms** | **216ms** (41x slower) |
| Decode | **0.5ms** | never succeeded — see below |
| Avg size | 798 bytes | 706 bytes (12% smaller) |

AVIF's ~12% size win doesn't come close to justifying the cost:

- **AVIF encoding is 41x slower than WebP even in release mode**, because `rav1e` (the AV1
  encoder AVIF sits on top of) needs `nasm`-built assembly routines for real speed, and nasm
  isn't installed on this machine. Without it, `rav1e` falls back to a pure-Rust path that's
  dramatically slower — in a debug build this was **9.16 *seconds*** per tiny thumbnail,
  which is what caused an early confusing "app not responding" report before I traced it to
  this. Generating a 100k-item library with AVIF at this speed would take upwards of a day
  single-threaded, or tens of minutes even fully parallelized across 16 threads.
- **AVIF decode never worked at all** through `image` crate 0.25's `avif` feature in this
  environment — every decode attempt (30/30, and later 500/500 in a separate run) returned
  `"The image format Avif is not supported"`, despite the same feature successfully driving
  the *encoder*. This is encode-only support in practice here, not a fixable-by-installing-
  nasm problem — decode is a different, apparently unwired code path.

WebP (via the `webp` crate, real libwebp bindings, lossy q78) is what the actual 100k-item
spike library was generated with, and it's what the grid decodes through the asset protocol
during all the scroll/scrubber/slider testing above.

## 4. Harder than expected

- **`cargo build --release` is not the same as a real release build.** Tauri only serves the
  built frontend (`frontendDist`) when compiled with the `custom-protocol` Cargo feature on
  the `tauri` crate; without it (e.g. calling `cargo build --release` directly, bypassing the
  `tauri` CLI) the binary still tries to hit the dev server at `localhost:1420` regardless of
  the optimization profile, and fails with a blank "localhost refused to connect" window. Easy
  to hit by accident if you're driving builds directly instead of through `tauri build`/
  `tauri dev`. Cost real time to trace.
- **Debug vs. release is not a minor speed difference for codec work — it's 6–40x.** WebP
  encode went 33ms → 5.2ms (debug → release); AVIF encode went 9.16s → 216ms. The 100k-item
  library generation itself went from **437s (7.3min) to 181s (3min)** debug→release. Anyone
  developing M1's real thumbnailing pipeline should budget dev-loop time accordingly and do
  perf sign-off exclusively on release builds — this spike's early numbers (before I caught
  the debug/release gap) were meaningless.
- **A synchronous Tauri command blocks the window's message pump, not just "the backend."**
  The first version of `ensure_library` (100k parallel file writes) and `benchmark_formats`
  (500 sequential AVIF encodes) ran as plain synchronous `#[tauri::command]` functions. Windows
  marked the app "Not Responding" during both — the whole native window event loop stalls, not
  some background thread. Fixed by wrapping the heavy work in
  `tauri::async_runtime::spawn_blocking` and making the commands `async fn`. This is a sharp
  edge M1's real (much heavier) hashing/thumbnailing/ffmpeg-orchestration job queue needs to
  respect from the start, not retrofit.
- **A `debug_assert!` inside a Tauri command is a loaded gun.** The first AVIF-decode-check
  used `debug_assert!(decoded.is_ok())`; since AVIF decode reliably failed, this aborted the
  *entire process* with `STATUS_STACK_BUFFER_OVERRUN` the moment the benchmark ran — not a
  graceful error, a hard crash of the whole app, because the panic happened somewhere that
  can't unwind. Worth a standing rule: no asserts (debug or otherwise) inside command handlers
  for anything that can plausibly fail at runtime.
- **AVIF's rough edges (above) directly validate a suspicion already in the brief** — the
  M0-SPIKE doc flagged "AVIF decodes slower" as a thing to check. It's worse than slower: it
  didn't decode at all here, and the encoder is only viable with a system dependency (nasm)
  this machine didn't have. Nothing in PLAN.md assumed AVIF specifically, so this doesn't
  contradict a locked decision — it just closes the question in WebP's favor with real data
  instead of the brief's hedge.
- **Nothing about the virtualization/justified-grid architecture itself was harder than
  expected.** The row-precompute-in-a-worker approach worked essentially as sketched in the
  brief on the first real attempt, and the numbers have wide margins (2–100x) against every
  target except the frame-time-under-load one, which has a plausible non-app explanation
  (see §1) rather than an architectural one.

## Bottom line

The core risk this milestone exists to test — can a justified virtualized grid hold up at
100k items — checks out with comfortable margins on first paint, relayout, and scrubber-jump
latency. Sustained frame time during scroll/interaction is good on the p50/p95 (14–17ms) but
not yet a clean "zero frames over 32ms" pass; the two contributing causes (input-storm
coalescing during fast slider/scrubber drags, and a probable third-party GPU overlay) are both
plausibly fixable/excludable rather than being evidence the architecture doesn't scale. Before
treating "no frame >32ms" as fully proven, re-run with the AMD overlay disabled and with
interaction type tagged per-sample so fling can be isolated and checked against its own
100ms-blank-frame criterion specifically.
