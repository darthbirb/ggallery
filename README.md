# GGallery

A local-first media library for Windows. Folders on disk are the organising truth; tags,
labels and search sit on top. Single user, runs locally, no network service of any kind.

It exists because nothing else does all of this at once: folder-based *and* tag-based
organisation, one portable library folder, integrated downloads, a compression pipeline,
duplicate detection, and a triage flow fast enough to actually use.

> **Status: pre-alpha. There is nothing to run yet.**
> M0 (the grid performance spike) is complete and its findings are recorded. M1 is the
> first milestone that produces a usable application.

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

### Building

Nothing to build yet. Once M1 lands:

```bash
npm ci
```

```bash
npx tauri dev
```

Always build and measure through the `tauri` CLI. `cargo build --release` on its own
produces a binary that still points at the dev server — see
[docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md).

---

## Repository layout

```
CLAUDE.md                  instructions for Claude Code — read first
PLAN.md                    stack, locked decisions, non-goals, roadmap
README.md                  this file
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
| M1 | Core library — index, hash, thumbnails, job queue, grid. Read-only | Next |
| M1.5 | First-import wizard — the UUID rename, with dry run and reversal | |
| M2 | Folders as entities — archetypes, labels, tag inheritance | |
| M3 | Search — query parser, sectioned results | |
| M4 | Sorting Box and triage — hotkey culling, undo, trash | |
| M5 | Downloads — yt-dlp and gallery-dl integration | |
| M6 | Compression and review — side-by-side comparison | |
| M7 | Duplicates — perceptual hashing, tag merging | |
| M8 | Utility screens — storage, tag management, export, integrity | |
| M9 | Polish — command palette, settings, blur toggle | |

M4 is the milestone that replaces the manual drag-and-drop sorting this project exists to
kill. Everything before it is groundwork.

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

## Renaming

This repository is still named `gallery` on disk. To finish the rename:

```bash
git -C D:/Projects/gallery remote -v
```

Then close all editors and terminals pointing at it, and:

```bash
mv /d/Projects/gallery /d/Projects/GGallery
```

Nothing inside the repo hardcodes the folder name — the Claude hook resolves through
`$env:USERPROFILE`, and the docs use relative links — so the rename is the only step.
