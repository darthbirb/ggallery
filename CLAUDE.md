# GGallery

A gallery viewer and collection organiser for Windows, in that order. The main activity is
looking at things — browsing, opening, comparing. Folders, tags, search, downloads,
compression and triage all exist to serve that, not the other way round.

Folders on disk are the organising truth; tags, labels and search sit on top. One library
folder holds everything, so backup is "copy the folder".

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

  This governs **the shipped application at runtime**, not the build toolchain. Cargo,
  npm and rustup keep machine-wide caches (`~/.cargo/registry`, the npm cache) by design
  and are shared across every project — that is normal and correct. Do not try to relocate
  them into the repo; setting `CARGO_HOME` locally would bloat the repository and slow
  every build for no benefit.
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

## Running commands

**Never prefix a command with `cd` or `Set-Location`.** The working directory is already
the project root. Permission rules match on command *prefixes*, so
`Set-Location D:\Projects\ggallery; git status` matches no rule at all and prompts, while
a bare `git status --short` is already allowed. Prefixing defeats the entire allowlist.

For the same reason, keep commands single where it costs nothing. Chaining unrelated
work with `&&` or `;` produces a string that matches nothing and prompts, even when every
part of it is individually allowed.

## Verifying the app

**Never drive the user's mouse or keyboard.** Do not move the cursor, click at guessed
coordinates, or send keystrokes to a running window. It is unreliable — coordinates drift,
the wrong control gets focus, and the result is a confident report of something that never
happened — and it takes over a machine someone else is using.

Launching the built binary and taking a screenshot is fine. Interacting with it is not.
When a milestone needs interactive behaviour confirmed, list the specific steps and ask
the user to run them.

## Finishing a milestone

**Always run a full release build before reporting a milestone done** — frontend included,
through the `tauri` CLI:

```
npm run tauri build
```

`cargo check`, `cargo test` and `tsc --noEmit` passing is not the same as the application
building and running. LTO and `codegen-units = 1` make this a several-minute build; run it
in the background and report the result, including whether the binary actually launches
against a real library.

## Working style

Build one milestone at a time, in order. Do not start the next one or add features from
it because they seem convenient. If something in the specs looks wrong, say so before
building around it.
