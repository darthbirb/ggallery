# GGallery

A gallery viewer and collection organiser for Windows, in that order. The main activity is
looking at things — browsing, opening, comparing. Folders, tags, search, downloads,
compression and triage all exist to serve that, not the other way round.

Folders are data, not directories: the hierarchy lives in the database and every file sits
flat under `files/`, sharded by uuid. Single user, runs locally, no network service.

**The repository and the library are separate.** This repo is the application. The library
is a folder elsewhere on disk, chosen on first run and recorded in `gallery.config.json`
next to the exe. Never assume a path — resolve the library root from config, and never
commit anything referencing a specific machine's layout.

Stack: Tauri v2 · Rust · React + TypeScript + Vite · Tailwind v4 + `shadcn/ui` (Radix
underneath, copied into `src/components/ui/` and restyled) · SQLite (`rusqlite`, WAL) ·
ffmpeg, HandBrakeCLI, yt-dlp and gallery-dl as sidecar binaries.

## Which file to open

| | |
| --- | --- |
| [SPEC.md](SPEC.md) | What the app is and does. Behaviour, not appearance. |
| [docs/DECISIONS.md](docs/DECISIONS.md) | The numbered locked decisions. Code cites these by number. |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones, what is next, and what is deliberately excluded. |
| [docs/SCHEMA.md](docs/SCHEMA.md) | Tables, tag resolution, query language. |
| [docs/NOTES.md](docs/NOTES.md) | Gotchas that already cost time, grid architecture, module boundaries, on-disk layout. |
| [docs/design/](docs/design/) | **The interface.** `GGallery.dc.html` is the drawing; [SOURCE.md](docs/design/SOURCE.md) says how to read it. |
| [docs/design/DEVIATIONS.md](docs/design/DEVIATIONS.md) | Where we deliberately differ from the drawing, and what is not built yet. |
| [docs/NITPICKS.md](docs/NITPICKS.md) | Interface complaints, collected until M2.9 clears them. |

**The drawing wins.** `docs/design/GGallery.dc.html` is the specification for how the
application looks. Where older text disagrees — SPEC.md, a locked decision, a comment in
the code — build the drawing and amend the text. Do not weigh a conflict on its merits: the
older decision was made without the drawing. Everything we deliberately do differently is
in DEVIATIONS.md §1, and that list is closed at four entries; anything else that differs is
a bug or an unbuilt item, not a choice.

## Keeping the docs true

**Write the current state, not the journey.** If a milestone added a green button and a
later one made it blue, the spec says the button is blue. Nobody needs the green.

**Keep the lesson, drop the chronology.** Where the history *is* the warning — a query
shape that collapsed at 100k, a command that may have crashed the machine — the warning
survives without the story around it. The test is whether someone could repeat the mistake
by not knowing.

**Amend in place.** A reversed decision is rewritten, never appended to. Decision numbers
never change; code cites them.

## Rules that are easy to break silently

- **No absolute paths in the database, ever.** Everything relative to the library root,
  forward slashes, normalised case. `fs/paths.rs` is the only converter.
- **Nothing is written outside the app directory and the library root** at runtime. No
  registry, no `%APPDATA%`, no installer. The app must run from a USB stick.

  This governs the **shipped application**, not the build toolchain. Cargo, npm and rustup
  keep machine-wide caches by design; do not try to relocate them.
- **WebView2's data directory must be redirected into the app directory.** Tauri defaults
  it to `%LOCALAPPDATA%\<bundle-id>\`, silently breaking the rule above.
- **Files on disk are named `<uuid>.<ext>`.** The app owns filenames. Original names are
  database metadata.
- **Destructive operations need an undo path** — moves, deletes and compressions go through
  the journal so `Ctrl+Z` survives a restart. **Renames are the exception**: the app owns
  filenames, so renaming is normalisation, and there is no reversal tooling.
- **Heavy Tauri commands are `async fn` + `spawn_blocking`.** A synchronous command blocks
  the window message pump and Windows marks the app "Not Responding".
- **No `assert!` or `debug_assert!` in a command handler.** A failed assert aborts the
  process. Return `Result`.
- **Measure on release builds only**, through the `tauri` CLI. Debug is 6–40× slower on
  codec work, and `cargo build --release` alone still points at the dev server.

## Nothing changes the machine

**The blast radius of any command stops at this repository and the library folder.** No
registry edits, services, scheduled tasks, drivers, global installs, environment changes
outside the process, `fsutil`, `diskpart`, `bcdedit`, `netsh` or `reg`. Also the
non-obvious: changing how a filesystem behaves, altering directory permissions, disabling
something to make a test pass.

**Setting up a test is never a reason to reconfigure the machine.** One milestone reached
for `fsutil file setCaseSensitiveInfo` so a sibling-merge test could run against real
case-differing directories. The machine then crashed twice and a cleanup command hung for
two minutes. Causation was never proved and does not need to be — the test was rewritten to
prove the same logic without touching the filesystem, which is what it should have done
first. A test that seems to require a system-level change is reaching too far.

If something genuinely cannot be done without touching the machine, say so and let the user
decide. It is their computer and they are sitting at it.

## Git

Committing and pushing are the user's decisions, always. Show what would go in, then wait
for an explicit yes — never commit as a convenient last step. `git clean`, force push,
`git reset --hard` and history rewriting are off the table; if one seems necessary, say so
and let the user run it.

Commit messages are a **single lowercase subject line**. No body, no trailers.

## Running commands

**Never prefix a command with `cd` or `Set-Location`.** The working directory is already
the project root, and permission rules match on command *prefixes* — prefixing defeats the
whole allowlist. For the same reason, keep commands single where it costs nothing.

## Verifying

`npm run dev` then `http://localhost:1420/#kitchen-sink` renders every primitive in every
state on one page — dev-only, no library or config needed. Anything that changes a
control's look is checked there first.

**Screenshots do not work in this environment.** The Browser pane is not displayed, so
every capture times out; `read_page`, `javascript_tool` and the console all work. Verify
appearance through computed styles and leave the looking to the user.

**Never drive the user's mouse or keyboard.** Launching the built binary is fine;
interacting with it is not. When a milestone needs interactive behaviour confirmed, list
the steps and ask the user to run them.

**Always run a full release build before reporting a milestone done** — `npm run tauri
build`, frontend included. `cargo check`, `cargo test` and `tsc --noEmit` passing is not
the same as the application building and running. LTO makes this a several-minute build;
run it in the background and report whether the binary actually launches.

## Working style

One milestone at a time, in order. Do not start the next one or pull features back from it.
If something in the specs looks wrong, say so before building around it.

**How much to ask.** Where two options are genuinely comparable and the choice is taste,
ask, and present the tradeoff rather than steering. Where one is clearly better, recommend
it and confirm — do not silently decide. Where a convention obviously applies, decide
quietly. Never make a blind choice on anything that shapes how the app is used.

**Do not invent a layout for an ordinary surface.** The drawing covers most of them. Where
it is silent — DEVIATIONS.md §5 lists what it does not say — a citation has to be lookable
rather than recalled: a screenshot in `docs/reference/`, or a `shadcn/ui` block read
through the registry MCP server, not a remembered application name.
