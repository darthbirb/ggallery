# Product

## Platform

web

## Users

One user: the owner, on their own Windows 11 machine, single 1440p display at 100%
scaling. There is no second audience — no team, no sharing, no accounts, no support
burden. Design decisions do not have to survive a stranger's first five minutes; they have
to survive daily use by someone who knows exactly what everything does.

The job is looking at a large personal media library — browsing, opening, comparing — and
keeping it organised enough that anything can be found again. Sessions are long and
repetitive: the same controls are used hundreds of times an hour.

## Product Purpose

A gallery viewer and collection organiser for Windows, **in that order**. The main activity
is looking at things. Folders, tags, search, downloads, compression, duplicate detection
and triage exist to serve that, not the other way round.

Success is that the library stays worth looking at: filing never becomes the reason to stop
adding to it, and nothing tedious enough to avoid stands between having a file and finding
it later.

**The stated tiebreaker:** when a decision trades off between making the app better to look
through and better to administer, looking through wins.

## Positioning

It exists because nothing else does all of it at once: a fast viewer over a large local
library, folder-based *and* tag-based organisation, one portable folder that backs up by
copying, integrated downloads, a compression pipeline, duplicate detection, and a triage
flow quick enough to keep up with.

It replaces a manual workflow assembled from parts — folders named by hand, HandBrake for
compression, a dedupe tool, rename scripts, and dragging between two Explorer windows.

## Operating Context

- **Library scale:** 20,000–100,000 items. Verified against a synthetic 100k library.
- **The repository and the library are separate.** The app is a portable `.exe`; the
  library is a folder elsewhere on disk, chosen on first run, recorded in
  `gallery.config.json` next to the exe. Never assume a path.
- **Folders are data, not directories.** The hierarchy lives in the database; every file
  is stored flat under `files/`, sharded by its uuid. Tags, labels and search sit on top.
  A plaintext `library.jsonl` plus rolling database backups are the redundant copy the
  directory tree used to be.
- **One library folder holds everything**, so backup is "copy the folder".
- **Runs from a USB stick.** Nothing is written outside the app directory and the library
  root — no registry, no `%APPDATA%`, no installer.
- **Single window, always.** Comparison happens in the split pane, never a second window.
- **No network service of any kind.** Downloads reach out; nothing listens.

## Capabilities and Constraints

Tauri v2 · Rust · React + TypeScript + Vite · Tailwind + `shadcn/ui` (Radix underneath,
copied into `src/components/ui/` and restyled) · SQLite (`rusqlite`, WAL) · ffmpeg,
HandBrakeCLI, yt-dlp and gallery-dl as sidecar binaries.

Constraints that are not negotiable and that shape the interface:

- **Files on disk are named `<uuid>.<ext>`.** The app owns filenames completely; original
  names are searchable metadata. This is why *Reveal in Explorer*, *Copy file* and *Export*
  matter more here than in an ordinary viewer.
- **No absolute paths in the database.** Everything is relative to the library root.
- **Destructive operations need an undo path**, journalled so it survives a restart.
  Renames are the deliberate exception.
- **Heavy work is `async` + `spawn_blocking`**; a synchronous command freezes the window.
- **Grid performance is a hard requirement**, validated in a spike: a recycled DOM tile
  pool, precomputed row breaks in a worker, binary search on row offsets.

Terminology used in the product: *Everything*, *Sorting Box* (the library root itself, not
a directory), *Favourites*, *the pane*, *the folder band*, *Pending Review*, *Trash*.

**The app ships with no domain vocabulary.** No seeded archetypes, no named field types, no
folder-name conventions. Every example in the specs is an illustration, never a string in
the product.

## Brand Commitments

- **Name:** GGallery. Fixed.
- **Mark:** none exists. To be designed — must read at 16–20px in a title bar and work as
  the Windows `.ico`.
- **One accent, chosen from a fixed set** — Azure (default), Steel, Teal, Indigo. Green
  and red are reserved for meaning (kept, saved, deleted, failed) and are never the
  accent, and nor is amber, which means unfinished.
- Dark, dense, quiet. The interface recedes so the media does not compete with it.

## Evidence on Hand

- A real library on the user's disk, plus a synthetic 100k-item generator
  (`src-tauri/src/bin/synth_library.rs`) for scale work.
- Measured performance results in `docs/M0-RESULTS.md`.
- A dev-only component gallery at `#kitchen-sink` showing every primitive in every state.
- **No users, testimonials, press, pricing, licensing or deployment story exist**, and none
  may be invented. This is a personal tool built in the open, not accepting contributions.

## Product Principles

1. **Looking through beats administering.** Every surface is judged by whether it makes the
   library better to look at.
2. **Nothing is keyboard-only.** Every action has a visible control; keys are a second path
   to something already on screen. The user is explicitly mouse-first.
3. **There is no polish phase.** Each milestone ships at the standard, because deferred
   polish is abandoned polish.
4. **Do not invent a layout for an ordinary surface.** The viewer is designed here;
   everything else cites prior art that can be looked at rather than recalled.
5. **Every noun needs a full lifecycle** — created, renamed, moved, deleted — written down
   as operations, not implied by a future context menu.

## Accessibility & Inclusion

No clinical requirement established. Two product-specific needs are confirmed:

- **Mouse-first.** The user dislikes keyboard-driven flows; a capability reachable only by
  shortcut is unfinished.
- **Controls sized to be hit and seen.** Heights 28/32/38px, icon buttons never below
  32×32, glyph filling 55–60% of its button, every button with a visible surface at rest.
  Base UI text 14px, mono 12px.
