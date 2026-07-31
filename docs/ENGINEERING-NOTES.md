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

**Two known follow-ups:**

- **Coalesce pointer-drag input.** Single scrubber jumps paint in 0.7ms (p50). A
  *continuous stream* of them during a fast drag saturates the frame budget and produces
  40–110ms hitches. Debounce or coalesce drag-driven relayout specifically; the discrete
  jump case needs nothing.
- **React reconciliation is the ceiling, not layout.** The visible-range recompute is
  React-state-driven, which is fine at 100k with light tiles. Once tiles carry favorite
  badges, selection state and hover-scrub, watch for it — imperative DOM recycling is the
  escape hatch if it degrades.

---

## Open from M0

Two measurements were not cleanly obtained and should be redone before the frame-time
target is treated as fully proven:

1. **Re-run with AMD Adrenalin's overlay disabled.** Isolated 85–110ms stalls recurred
   every 20–60 seconds *including at idle* with a flat heap — not GC. `AMDRSServ.exe` hooks
   the present/swap-chain path, and WebView2 is a normal Chromium GPU client. Probable
   cause, unconfirmed.
2. **Tag frametime samples by interaction type.** The M0 session mixed slow scroll, fling,
   slider drag and scrubber drag without labelling which window belonged to which, so the
   "no blank frame >100ms during a top-to-bottom fling" criterion was never checked against
   a fling specifically.

Excluding the uncorrelated stalls, interaction-driven frame times were comfortably inside
budget at p95 16.9ms.
