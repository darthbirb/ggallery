# Gallery — Plan

A local-first media library for Windows. Folders on disk are the organising truth;
tags, labels and search live on top. One root folder holds everything, so backup is
"copy the folder".

- [docs/DESIGN.md](docs/DESIGN.md) — product and UX specification
- [docs/DATA-MODEL.md](docs/DATA-MODEL.md) — schema, tag resolution, query language

---

## Stack

| Layer | Choice | Why |
| --- | --- | --- |
| Shell | Tauri v2 | WebView2 ships with Win11 → standalone ~10MB exe, no runtime install |
| Backend | Rust | Parallel hashing, thumbnailing, ffmpeg orchestration |
| Frontend | React + TypeScript + Vite | Best virtualization ecosystem, largest answer pool |
| Styling | Tailwind + Radix primitives | Professional look without fighting a component library |
| Database | SQLite (`rusqlite`, WAL) | Single file, lives in the root folder |
| Media | ffmpeg, HandBrakeCLI, yt-dlp, gallery-dl | Sidecar binaries in `tools/` |

## Locked decisions

1. **Folders are entities.** Title, optional archetype, labelled fields, free tags,
   cover image. Searchable and taggable in their own right.
2. **Tags are inherited live.** An item's effective tags are recomputed from its
   current location every time it moves. No accumulation, no drift.
3. **Two tag shapes.** Labels (`instagram: @ana`) and flags (`beach`).
4. **Archetypes are folder templates.** "Person" pre-creates empty `instagram`,
   `tiktok`, `youtube` fields that stay visible while unfilled.
5. **Filenames are UUIDv4** plus the real extension. Original filename is kept in
   the database as searchable metadata.
6. **Sorting Box** is a watched folder at `<root>/Sorting Box/`. Files arrive via the
   app, Windows drag-and-drop, downloads, or being pasted in from Explorer.
7. **Triage is fullscreen-first**, one item at a time with destination hotkeys. Grid
   multi-select mode is one keystroke away.
8. **Every compression is reviewed manually** before replacing anything.
9. **Replaced originals go to trash**, purged on demand with a visible reclaimable
   space figure.
10. **Folder views are recursive** by default, with a "this folder only" toggle.
11. **Nothing is written outside the app directory and the root folder.** No registry,
    no `%APPDATA%`, no installer.
12. **Favorite is first-class, not a tag.** One key, a badge on the thumbnail, a
    permanent sidebar entry. Binary — no star ratings, no colour labels.
13. **Folders carry a status** — Active / WIP / Done / Archived — plus a tracked
    "last added" date, so WIP becomes a staleness-sorted to-do list rather than a
    label you forget you set.
14. **Filenames are opaque, so export exists.** Selecting items and exporting them
    reconstructs meaningful filenames into a chosen location.

## Non-goals

Deliberately excluded. Do not build these without an explicit decision to reverse:

- **Subscriptions / auto-checking sources for new content.** Downloads are manual:
  paste a URL, it downloads. Per-item download history is still recorded and
  searchable, but nothing runs on a schedule.
- **Collections / albums.** Ordered curated sets spanning folders. Saved searches plus
  favorites cover the need; revisit only if that proves false in use.
- **Star ratings and colour labels.** Favorite is binary.
- **Hidden or PIN-protected folders.** A blur toggle is the only privacy affordance.
- **Face recognition.** Possible far-future addition; nothing in the model should
  block it, nothing should assume it.
- **Any network service, sync, or sharing.** This is a local single-user application.

## Portability

The repo is for development. CI builds a portable `.exe` and publishes it to GitHub
Releases. Moving machines is: download the exe, copy the root folder, point the app at
it.

```
Gallery/                    ← anywhere, USB stick included
  gallery.exe
  gallery.config.json       ← which root folder, window state
  tools/                    ← fetched on first run, gitignored
```

`tools/` is gitignored and fetched on first run with pinned versions and checksum
verification. The same code path serves the "Update tools" button, which is required
rather than optional — yt-dlp breaks against sites every few weeks.

**Absolute paths must never reach the database.** Everything is relative to root,
forward slashes, normalised case. This is the single rule that keeps portability alive.

## On-disk layout

```
<root>/
  .gallery/
    library.db            ← SQLite, WAL, checkpointed on exit
    library.jsonl         ← plaintext export, disaster recovery
    cache/
      thumbs/ab/cd/<uuid>.avif
      sprites/ab/cd/<uuid>.webp    ← 10-frame scrub strip per video
    trash/                ← soft-deleted files, rel_path preserved
    pending/              ← compressed candidates awaiting review
    lock                  ← single-instance guard
  Sorting Box/
  People/
    ana/
  Places/
```

Cache runs ~4–6GB at 100k items. It stays inside root so that copying the folder gives
a working library immediately rather than a 30-minute thumbnail rebuild. One setting
relocates it; it is safe to delete at any time.

`library.jsonl` is written on a debounce, one line per item keyed by UUID, carrying
folder path, tags and labels. If the database ever corrupts it rebuilds from a file you
can read in Notepad.

---

## Roadmap

### M0 — Grid spike (throwaway)

Generate 100k synthetic thumbnails. Build the justified virtualized grid. Confirm 60fps
scroll and smooth video hover-scrub. **Do this before committing to anything else** —
it is the only real technical risk in the project, and it is cheap to test.

Full brief with measurable pass criteria: [docs/M0-SPIKE.md](docs/M0-SPIKE.md).

### M1 — Core library

Root picker, filesystem walk, BLAKE3 hashing, metadata extraction, thumbnail and sprite
generation, persistent job queue, folder tree, the real grid.

**First-import migration** needs care and is specified in
[docs/DESIGN.md](docs/DESIGN.md#first-import) — renaming an entire existing library to
UUIDs is the most destructive thing this app will ever do.

### M2 — Folders as entities

Folder records, titles, archetypes, labels, flags, tag inheritance and the materialised
effective-tag cache. Folder header UI and the inspector panel. Favorites, folder status,
notes. Lightbox viewer and the timeline scrubber.

### M3 — Search

Query parser, unified search bar, sectioned results (folders then media), saved
searches, FTS index.

### M4 — Sorting Box and triage

Fullscreen culler with bindable destination hotkeys, grid multi-select mode, inline
folder creation, drag-and-drop from Windows, filesystem watcher, undo journal, trash.

**This is the payoff milestone.** Resist adding anything before it.

### M5 — Downloads

URL → tool detection → download into Sorting Box, auto-labelling with source and
uploader, archive-file integration so nothing downloads twice, queue view, tool updater,
cookie support (Instagram will not work without it).

### M6 — Compression and review

Preset management, HandBrakeCLI and image encoding jobs, Pending Review queue,
side-by-side comparison for images and video, lineage, trash integration.

### M7 — Duplicates

Perceptual hashing, grouping, side-by-side comparison, tag merging from loser to keeper.

### M8 — Utility screens

Storage dashboard, tag management (rename, merge, aliases, unused), export with
reconstructed filenames, integrity check.

### M9 — Polish

Command palette, settings, keyboard reference, blur toggle, `library.jsonl` export and
rebuild, backup verification.
