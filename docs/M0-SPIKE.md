# M0 — Grid performance spike

**Throwaway code.** Build it in `spike/`, prove or disprove the numbers, write up the
result, then delete it. Nothing here is meant to survive into the real app.

## Why this exists

The whole application is a virtualized media grid with panels around it. If that grid
cannot hold 60fps at full library scale, the architecture in
[../PLAN.md](../PLAN.md) needs to change — and it is far cheaper to find that out now
than after the schema, the tag system and the triage flow are built on top of it.

Target scale: **100,000 items**, roughly half images and half video. That is double the
realistic ceiling, deliberately.

## What to build

A Tauri v2 app (React + TypeScript + Vite) that does exactly four things.

### 1. Generate a synthetic library

A Rust command that writes **100,000 real thumbnail files** to disk — small WebP or AVIF,
varied aspect ratios (portrait, square, 4:3, 16:9), a few KB each, in a `ab/cd/` hash
fan-out. Also generate **2,000 video sprite strips** (10 frames wide) for the hover-scrub
test.

> Generating real files matters. A spike that renders 100k CSS gradients proves nothing —
> the thing under test is decoding and loading real images through Tauri's asset protocol
> under scroll pressure. Fake tiles would pass a test the real app then fails.

Keep a flat in-memory list of `{ id, width, height, kind, capturedAt }` as the stand-in
for the database. **Do not build a schema.**

### 2. A justified virtualized grid

Rows fill edge to edge at a target row height, images keeping their aspect ratio — the
Google Photos / Flickr layout.

Suggested approach, but prove it rather than assuming it:

- Precompute row breaks for all 100k items **once**, off the main thread (web worker, or
  in Rust and send the result over). This yields an array of row offsets and heights.
- Virtualize by row using absolute positioning against that precomputed layout, so the
  scroll container has a real fixed height and the scrollbar behaves natively.
- Recompute row breaks on viewport resize and on thumbnail-size change, debounced.

Include the thumbnail size slider — it forces a full relayout and is the worst case.

### 3. Timeline scrubber

The strip down the right edge. Dragging it must jump to an arbitrary index and paint
immediately. This is the test that catches layout schemes which only work when you arrive
somewhere by scrolling.

### 4. Video hover-scrub

Hovering a video tile scrubs through its sprite strip following the cursor. Test it while
scrolling, on tiles that are entering and leaving the viewport.

## Pass criteria

Measure and record actual numbers, not impressions.

| | Target |
| --- | --- |
| Sustained scroll | 60fps, no frame over 32ms during a slow drag-scroll |
| Fling top→bottom | no blank frame held longer than 100ms |
| Time to first grid paint | under 1s with 100k items loaded |
| Scrubber jump to arbitrary index | painted under 100ms |
| Thumbnail size change (full relayout) | under 250ms |
| Memory after scrolling the full library twice | stable, no unbounded growth |
| Idle memory with 100k items | under 500MB |

Use the DevTools performance panel and a `requestAnimationFrame` frame-time logger. Test
on the machine this will actually run on, not a beefier one.

## Explicitly out of scope

No SQLite. No tags, folders, archetypes, or inheritance. No sidebar, inspector, search,
or triage. No real media files. No styling beyond what is needed to see the layout — the
visual design is settled in [mockup.html](mockup.html) and is not what is being tested.

## Deliverable

A short `spike/RESULTS.md` recording:

1. The measured numbers against each target above.
2. Which virtualization approach was used, and what was tried and rejected.
3. Thumbnail format and size that worked best (WebP vs AVIF — AVIF decodes slower, and
   at thumbnail dimensions the size win may not be worth it).
4. Anything that was harder than expected, and any assumption in
   [../PLAN.md](../PLAN.md) this contradicts.

If the numbers miss, say so plainly and describe where the time went. **A failed spike is
a successful M0** — it is doing its job either way.
