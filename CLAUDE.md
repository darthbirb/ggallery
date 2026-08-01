# GGallery

A local-first media library for Windows. Folders on disk are the organising truth; tags,
labels and search sit on top. One library folder holds everything, so backup is "copy the
folder".

Single user, runs locally, no network service of any kind.

**The repository and the library are separate.** This repo is the application. The
library is a folder elsewhere on disk, chosen by the user on first run and recorded in
`gallery.config.json` next to the exe. Never assume a path — always resolve the library
root from config, and never commit anything that references a specific machine's layout.

## Read before working

- [PLAN.md](PLAN.md) — stack, locked decisions, non-goals, roadmap
- [docs/DESIGN.md](docs/DESIGN.md) — product and UX specification
- [docs/DATA-MODEL.md](docs/DATA-MODEL.md) — schema, tag resolution, query language
- [docs/STRUCTURE.md](docs/STRUCTURE.md) — where every file goes, and the module
  boundaries that are not negotiable
- [docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md) — Tauri/Windows gotchas that
  already cost time once, and the validated grid architecture
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
- **Destructive operations need an undo path.** Moves, deletes and compressions go through
  the journal so `Ctrl+Z` works across restarts. **Renames are the exception** — the app
  owns filenames, so renaming is normalisation rather than a user decision. Original names
  are kept in `item.orig_name` as metadata, and there is no reversal tooling.
- **WebView2's data directory must be redirected into the app directory.** Tauri defaults
  it to `%LOCALAPPDATA%\<bundle-id>\`, which breaks the rule above silently.
- **Heavy Tauri commands are `async fn` + `spawn_blocking`.** A synchronous command blocks
  the native window message pump and Windows marks the app "Not Responding".
- **No `assert!` or `debug_assert!` in command handlers.** A failed assert aborts the whole
  process instead of returning an error. Return `Result`.
- **Measure performance on release builds only**, built through the `tauri` CLI. Debug is
  6–40x slower on codec work, and `cargo build --release` alone produces a binary that
  still points at the dev server.

## Git

Committing and pushing are the user's decisions, always. Show what would go in, then wait
for an explicit yes — never commit or push as a convenient last step. `git clean`, force
push, `git reset --hard` and history rewriting are off the table entirely; if one seems
necessary, say so and let the user run it.

## Working style

Build one milestone at a time, in order. Do not start the next one or add features from
it because they seem convenient. If something in the specs looks wrong, say so before
building around it.
