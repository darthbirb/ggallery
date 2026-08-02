# GGallery

A gallery viewer and collection organiser for Windows, in that order. Single user, runs
locally, no network service of any kind.

**The main activity is looking at things** — browsing, opening, comparing. Folders and tags
exist so you can find something again. Downloads, compression, duplicate detection and
triage exist so the collection stays worth looking at. All of it serves the viewing, not
the other way round.

It exists because nothing else does all of that at once: a fast viewer over a large local
library, folder-based *and* tag-based organisation, one portable folder you can copy to
back up, integrated downloads, a compression pipeline, duplicate detection, and a triage
flow quick enough that filing never becomes the reason you stop adding to it.

> **Status: pre-alpha, but it runs.**
> The core library layer is built through M1.8. The app opens a library folder, imports
> it, indexes it, and scrolls the grid — and the library stays live: files added, changed
> or removed on disk by any means show up without a refresh and without a re-index button.
>
> Nothing above the file layer exists yet. No folders-as-entities, no tags, no search, no
> triage. That starts at M2.
>
> **Opening a library it has never seen renames every file to a UUID first, then
> indexes it** — a full-window Choose folder → Review → Progress flow, gated behind one
> backup acknowledgement because there is no undo. Original filenames live on as
> searchable metadata (`item.orig_name`); the uuid-to-original mapping is also kept in
> `library.jsonl` as a disaster-recovery export, but no tooling reconstructs names from
> it — there is no reversal feature. See [docs/DESIGN.md](docs/DESIGN.md#first-import).

---

## How it is meant to work

Two things, kept deliberately separate:

**The app** — this repository. Builds to a single portable `.exe`. Writes nothing to the
registry, nothing to `%APPDATA%`, and runs from a USB stick if you want it to.

**The library** — a folder somewhere else on your disk that holds all your media, chosen
by you the first time you run the app. Everything the app knows lives inside it, in a
`.gallery/` subfolder: the database, the thumbnail cache, the trash, the plaintext
backup export. Backing up is copying that one folder.

Moving to a new machine is therefore: download the exe, copy your library folder, point
the app at it. The path to your library is stored in `gallery.config.json` next to the
exe, which is gitignored precisely because it differs per machine.

```
GGallery/                     ← this repo, anywhere
  GGallery.exe
  gallery.config.json         ← points at your library (machine-local, not committed)
  tools/                      ← ffmpeg, HandBrakeCLI, yt-dlp, gallery-dl

D:\MyMedia\                   ← your library, anywhere else, chosen on first run
  .gallery/                   ← database, cache, trash — all app state lives here
  Sorting Box/
  People/
  Places/
```

---

## Getting set up

### Prerequisites

Windows 11 (WebView2 ships with it, so there is no runtime to install).

```bash
winget install Microsoft.VisualStudio.2022.BuildTools --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

```bash
winget install Rustlang.Rustup OpenJS.NodeJS.LTS
```

The Visual Studio Build Tools download is 2–3GB and is required — Rust needs the MSVC
linker on Windows. Restart your terminal afterwards so `cargo` and `npm` land on `PATH`.

### Sidecar binaries

`ffmpeg`, `HandBrakeCLI`, `yt-dlp` and `gallery-dl` live in `tools/` and are **not** in
git — they are around 140MB together. The app fetches pinned versions with checksum
verification on first run.

That same mechanism drives the **Update tools** button, which is a routine action rather
than a rare one: yt-dlp breaks against sites every few weeks and needs updating often.

The fetcher itself lands with the downloads milestone (M5). Until then, M1 looks for
`ffmpeg` and `ffprobe` in `tools/` and then on `PATH`, and says so in the window when it
finds neither: videos are still indexed, they just get no poster frame and no scrub strip.

### Building

```bash
npm ci
```

```bash
npx tauri dev
```

For a release binary:

```bash
npx tauri build --no-bundle
```

Always build and measure through the `tauri` CLI. `cargo build --release` on its own
produces a binary that still points at the dev server — see
[docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md).

Backend tests, including an end-to-end index of a scratch library:

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

### First run

Launch the exe and choose your library folder. The app creates `.gallery/` inside it,
walks the tree, and starts indexing: hashing each file, reading its dimensions and
capture date, and generating a thumbnail. The grid fills in as it goes, and the folder
tree and counts appear alongside it. Re-running the index later is incremental —
unchanged files are skipped by size and mtime.

---

## Repository layout

```
CLAUDE.md                  instructions for Claude Code — read first
PLAN.md                    stack, locked decisions, non-goals, roadmap
README.md                  this file
src/                       frontend — React + TypeScript
  features/grid/           the justified virtualized grid and its layout worker
  features/sidebar/        folder tree
  lib/                     ipc wrappers, shared types, formatting
  state/                   library and UI state
src-tauri/                 backend — Rust
  src/commands/            every #[tauri::command], and nothing else
  src/db/                  all SQL, plus numbered migrations
  src/fs/                  path normalisation and the library indexer
  src/media/               hashing, probing, thumbnails, scrub strips
  src/jobs/                the persistent job queue and its workers
  src/sidecar/             ffmpeg — the only thing that spawns processes
.claude/
  settings.json            permission rules (committed)
  hooks/guard.ps1          blocks destructive git, confirms commits and pushes
docs/
  DESIGN.md                product and UX specification
  DATA-MODEL.md            schema, tag resolution, query language
  STRUCTURE.md             where every file goes — spec, not description
  ENGINEERING-NOTES.md     Tauri/Windows gotchas, validated grid architecture
  M0-SPIKE.md              the grid spike brief
  M0-RESULTS.md            measured results from that spike
  mockup.html              visual reference — open it in a browser
```

---

## Roadmap

| | Milestone | State |
| --- | --- | --- |
| M0 | Grid performance spike | Complete — architecture validated, two defects located |
| M1 | Core library — index, hash, thumbnails, job queue, grid. Read-only | Built |
| M1.1 | M1 defects — index failures, stale state, scrollbars, context menu | Built |
| M1.5 | First-import wizard — the UUID rename, with dry run and verification | Built |
| M1.6 | Wizard placement, rename on arrival, dev-mode grid | Built |
| M1.7 | Import as a startup flow — rename before index, two screens, no reversal tooling | Built |
| M1.8 | The library is live — filesystem watcher, no re-index button | Next |
| M2 | Folders as entities — archetypes, labels, tag inheritance | |
| M2.1 | Folder and item operations — create, rename, move, delete | Next |
| M2.5 | The interface — designed from scratch, interactively | |
| M3 | Search — query parser, sectioned results | |
| M4 | Sorting Box and triage — hotkey culling, undo, trash | |
| M5 | Downloads — yt-dlp and gallery-dl integration | |
| M6 | Compression and review — side-by-side comparison | |
| M7 | Duplicates — perceptual hashing, tag merging | |
| M8 | Utility screens — storage, tag management, export, integrity | |
| M9 | Polish — command palette, settings, blur toggle | |
| M10 | Multi-view — up to twelve items playing at once in theatre view | |

**M2.5 is the milestone that decides whether this is a good viewer**, since the viewing
experience is the product. M4 removes the two-Explorer-window sorting chore, which is the
single most tedious thing the app replaces — but it is one job among several, not the
reason the app exists.

Full detail in [PLAN.md](PLAN.md). **PLAN.md also has a Non-goals section** — those
features are excluded deliberately, not overlooked.

---

## Working on this with Claude Code

Permissions are configured in two layers.

`.claude/settings.json` in this repo is committed, so the rules travel with the code. It
allows read-only inspection and ordinary build commands without prompting, requires
confirmation for anything that publishes or deletes, and denies reading credentials
outright.

`~/.claude/hooks/guard.ps1` is installed in your global Claude config and applies to
every project, not just this one. It scans the full command string rather than matching
prefixes, so `cd src && git push` is caught the same as a bare `git push`.

**Always confirmed with you:** commit, push, anything via `gh`, rebase, merge,
cherry-pick, revert, remote and config changes, installs, network fetches, deletes.

**Never permitted:** `git clean`, force push, `git reset --hard`, `git checkout -- .`,
history rewriting, and reading credentials, SSH keys or cookie files.

The hook fails open by design — if it errors, the declarative rules in `settings.json`
still apply. It is a second net, not the only one.

---

## Status

Pre-alpha and moving. M0 and M1 are done, the app indexes and browses a real library, and
nothing renames or deletes anything yet. See the roadmap above for where it goes next.

Not accepting contributions — this is a personal tool built in the open.
