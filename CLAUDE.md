# Gallery

A local-first media library for Windows. Folders on disk are the organising truth; tags,
labels and search sit on top. One root folder holds everything, so backup is "copy the
folder".

Single user, runs locally, no network service of any kind.

## Read before working

- [PLAN.md](PLAN.md) — stack, locked decisions, non-goals, roadmap
- [docs/DESIGN.md](docs/DESIGN.md) — product and UX specification
- [docs/DATA-MODEL.md](docs/DATA-MODEL.md) — schema, tag resolution, query language
- [docs/mockup.html](docs/mockup.html) — visual reference for layout and density

**PLAN.md has a Non-goals section.** Those features are excluded deliberately, not
overlooked. Do not build them.

## Stack

Tauri v2 · Rust · React + TypeScript + Vite · Tailwind + Radix · SQLite (`rusqlite`) ·
ffmpeg, HandBrakeCLI, yt-dlp and gallery-dl as sidecar binaries.

## Rules that are easy to break silently

- **No absolute paths in the database, ever.** Everything is relative to the library
  root, forward slashes, normalised case. This is the single rule that keeps the library
  portable between machines.
- **Nothing is written outside the app directory and the library root.** No registry, no
  `%APPDATA%`, no installer. The app must run from a USB stick.
- **Files on disk are named `<uuid>.<ext>`.** The app owns filenames completely. Original
  names are stored in the database as searchable metadata.
- **Destructive operations need a dry run and an undo path.** Moves, deletes, renames and
  compressions all go through the journal so `Ctrl+Z` works across restarts.

## Working style

Build one milestone at a time, in order. Do not start the next one or add features from
it because they seem convenient. If something in the specs looks wrong, say so before
building around it.
