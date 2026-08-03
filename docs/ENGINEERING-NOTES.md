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

## Tailwind v4

**`outline-none` plus `focus-visible:outline-*` on the same element cancels both.** Not a
subtle interaction — the control ends up with no focus ring at all, and because
`:focus-visible` is the only thing that ever draws one, the whole application looks like
Tab does nothing. Shipped that way in M2.5a; found in M2.5a.1.

Tailwind v4 compiles them to:

```css
.outline-none            { --tw-outline-style: none; outline-style: none }
.outline-2:focus-visible { outline-style: var(--tw-outline-style); outline-width: 2px }
```

`outline-none` sets the variable **unconditionally**, so the focus-visible rule resolves
its own style to `none`. Two further traps in the same area:

- A global `:focus-visible` rule in `@layer base` cannot rescue it. `.outline-none` is a
  utility, and the utilities layer wins.
- `outline-hidden` — the v4 rename people reach for instead — sets the same variable, so
  it fails the same way. shadcn's own components avoid this by using `ring-*` rather than
  `outline-*` for focus.

**What GGallery does instead:** one `:focus-visible` rule in `src/styles/index.css` and no
focus classes on controls at all. `outline-none` appears only on containers that take
programmatic focus and must never ring — a Radix menu surface, a dialog panel. Anything
needing an inset ring uses `focus-visible:-outline-offset-2` alone, which sets only the
offset and composes with the base rule.

`@property --tw-outline-style` is declared `inherits: false`, so this never leaks from a
parent to its children — it is strictly per-element, which is what makes the container
exception safe.

---

## Motion

All of this is from M2.5a.2, the pass that added animation. Decision 27 in
[PLAN.md](../PLAN.md) sets the policy; these are the mechanics that cost time.

**Do not use View Transitions to reflow a toast stack.** Tried it for the gap that closes
when one toast dismisses. It required delaying the actual dismiss until the transition had
captured, which put the state update behind an async boundary the auto-dismiss timer was
also racing — toasts stopped disappearing on their own, and the animation itself only played
sometimes. Reverted to plain immediate state updates. A stack that jumps closed is a far
smaller problem than a toast that never leaves, and toasts are the app's only visible undo
path, so they fail dangerous rather than ugly.

**Animate a panel closing by tweening its size, not by unmounting it.** Maximising the pane
originally unmounted the nav-and-grid side, which cannot animate — there is nothing left to
animate. The working shape is `flex-grow`/`flex-basis` tweened over 180ms with the
collapsing side fading and going `inert`, so it is untabbable and unclickable while it is
still on screen.

**A collapsible tree has to be nested in the DOM to animate.** The nav tree was built by
pushing every visible row into one flat array, which is the cheapest way to render a tree
and makes an expand animation impossible — a folder's children are siblings of the folder,
not children of anything that can be given a height. Rebuilt as recursive nesting so each
folder's children share one wrapper, which then reveals with `grid-template-rows: 0fr → 1fr`.
That transition is the one reliable way to animate to content height without measuring.

**A dialog backdrop that fades in flashes when one dialog replaces another.** The old
Settings opened a second dialog over itself; the backdrop unmounted and remounted, so it
undimmed and redimmed between them. Removing the backdrop's own entrance animation fixed the
flash. M2.5a.2 then removed the nested dialogs entirely, but the rule survives them: the
backdrop is a persistent surface, and animating it makes dialog *changes* visible as flicker.

**`prefers-reduced-motion` belongs in one global rule**, next to the single `:focus-visible`
rule, not as a `motion-reduce:` variant repeated on every animated class. The variant form
is only as complete as the last person's memory of it.

---

## shadcn/ui — audit vs adopt (M2.5a.3)

M2.5a hand-built every surface against Radix primitives without checking what shadcn's
registry ships composed. M2.5a.3 audited four of them — the navigation panel, the Settings
dialog, the resizable split, and the toaster — against the actual registry, read through
the MCP server in `.mcp.json`, not recalled. Recorded here so the next session that
notices "this app hand-rolled a sidebar" does not re-run the same audit from scratch.

**Navigation panel** (`features/nav/Nav.tsx`) — the registry has `sidebar` (the primitive)
and `sidebar-07` (collapses to icons). Both are built for a binary expanded/icon-rail
toggle with an off-canvas mobile variant; neither supports continuous drag-resize, the
animated recursive tree M0/M2.5a.2 required (`grid-template-rows` reveal — see *Motion*
above), per-row context menus, spring-loading drop targets, or decision 26's
filled-rounded-surface selection. Adopting would delete nothing and add a rewrite of all
of the above. **Stays hand-rolled.** Revisit only if the fold/resize/tree requirements
themselves change, not for tidiness.

**Settings dialog** (`features/settings/SettingsPanel.tsx`) — no composed "settings shell"
block exists, but `sidebar-13` ("a sidebar in a dialog") is shadcn's own reference answer
to this exact problem: `Dialog > Sidebar(collapsible="none") + content pane`, the same
shape `SettingsPanel` independently converged on. Adopting the actual `Sidebar` primitive
for a static four-row list would pull in `SidebarProvider` and its width/mobile-sheet
machinery to replace roughly fifteen lines of plain buttons — more code, not less. **Stays
hand-rolled**, but `sidebar-13` is now the citation DESIGN.md §2 was missing.

**Resizable split** (`components/Resizer.tsx`) — the registry's `resizable` wraps
`react-resizable-panels`: a panel-*group* that owns the whole layout and sizes panels in
percent. GGallery persists panel widths in **pixels** in `gallery.config.json`
(`commands/library.rs`), with a per-handle reset-to-default and a fixed arrow-key step.
Using the library would mean converting percent↔pixel on every window resize and
restructuring `App.tsx`'s flex layout into nested `PanelGroup`s for three independent
handles (nav, pane, filmstrip height). **Stays hand-rolled.** Would change if the app ever
needed a panel that resizes *proportionally* with the window rather than holding a fixed
pixel width — nothing in the current design asks for that.

**Toaster** (`components/Toaster.tsx`) — this is the closest call, not a clean keep.
`sonner` is the current shadcn answer; the old Radix-based `toast.tsx` is deprecated out of
the registry entirely. It genuinely targets the defect the *Motion* section above
documents: sonner owns the toast stack's mount/unmount and animates the gap closing
internally, which is exactly the reflow problem the abandoned View Transitions attempt
tried and failed to solve — the app currently ships the degraded "stack jumps closed"
behaviour as a deliberate trade-off, not a solved one. Sonner also supports
`toast.custom()` for fully custom JSX, so the real `Button` component could render as the
Undo action rather than sonner's own styled button, and it supports updating a toast in
place by reusing its `id` — so the "Moved 4 items…" → "Move undone" rewrite this app needs
is achievable on sonner, not blocked, contrary to a first pass at this audit.

What it would actually cost: `state/toasts.ts` is not only a render queue —
`App.tsx`'s global `Ctrl+Z` handler reads `toasts.toasts` directly to find the most recent
undoable toast. Sonner's internal store is not queryable from outside its own tree, so
adopting it would mean keeping `state/toasts.ts` as a parallel bookkeeping store anyway,
driving `toast.custom()` calls rather than being replaced by them — two stores kept in
sync, not one deleted. Net code change is close to a wash, weighed against a new
non-Radix dependency and unverified restyling of sonner's stack/expand/swipe behaviour to
the app's dense chrome. **Stays hand-rolled for now** — but this is the one surface where
adopting is a real, defensible answer if the stack-jump-closed behaviour ever becomes a
user-visible complaint, or if the undo accelerator's dependency on `toasts.toasts` goes
away. Not a "no block deletes code" case like the other three.

**The eleven `ui/` primitives** (button, badge, dialog, tooltip, select, checkbox, slider,
label, separator, input, and `menu.tsx`'s shared styling for dropdown-menu/context-menu)
were checked against current registry demo code component by component. Every divergence
found is a deliberate, documented app rule, not decay: button's variants (`accent` /
`danger` / `good`, no `ghost`) and sizes are decision 25 verbatim; badge's variants map to
the app's own tokens; `select.tsx` omits `SelectGroup`/`SelectLabel` only because nothing
in the app uses a grouped select yet; `menu.tsx` plus `components/Menu.tsx` intentionally
fuse `dropdown-menu` and `context-menu` into one definition (decision 23 — one menu,
complete in both places) and add `PointMenu`, which has no registry equivalent because the
grid's tiles are a recycled DOM pool, not React components (see *Grid architecture*
below). No `CheckboxItem`/`RadioItem` menu variants exist yet because nothing uses one.
None of this needs revisiting until a feature actually needs the missing piece.

---

## Dev-only code and the release bundle

**`import.meta.env.DEV` must be a literal at the call site or the code ships.** The
kitchen-sink route was first gated behind a hook that returned the flag through `useState`.
It typechecked, it passed every test, the page was unreachable in the built app — and the
whole of its markup was still in the release bundle, because Rollup can only eliminate a
branch it can constant-fold, and it cannot trace the flag through a hook's return value.

Verify by grepping the built JS for text that appears only on the dev surface, not by
checking the page is unreachable. Unreachable and absent are different things, and only one
of them is what "dev-only" means. Removing the indirection cut 6.5KB and left zero matches.

This applies to every flag of this shape, including the dev-mode grid.

---

## Filesystem watching

**A rename arrives as a `From`/`To` pair, and treating the halves independently destroys
data.** Windows emits both events adjacently for a single rename. Handled as an unrelated
removal followed by an unrelated creation, renaming a folder in Explorer retires every item
beneath it and re-indexes the same files as new ones — silently discarding tags, favorites,
notes and every other row keyed to the old identity.

The symptom appears long after the cause, looks like unexplained data loss, and is not
recoverable from anything the app keeps. Pair the events. Update the folder in place.

An unmatched `From` is a real removal — something left the watched tree — but it can only
be known to be unmatched once no `To` follows. Flush it when the next `From` arrives, or
after the settle window expires.

**Derive a moved folder's destination name from its title, never from `rel_path`.**
`rel_path` is stored lower-cased for comparison; using it as a source for a real directory
name silently lower-cases every folder that gets moved.

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
