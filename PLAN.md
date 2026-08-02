# Gallery — Plan

A gallery viewer and collection organiser for Windows, in that order.

**The main activity is looking at things.** Browsing the grid, opening an item, moving
through a folder, comparing two shots. Everything else exists to serve that: folders and
tags so you can find something again, search so you can find it faster, downloads so there
is more to look at, compression and duplicate detection so the collection stays worth
keeping, triage so filing does not become a chore that stops you adding to it.

A better media viewer, built for one person, that happens to also organise. Not an
organiser that happens to display files.

Folders on disk are the organising truth; tags, labels and search live on top. One root
folder holds everything, so backup is "copy the folder".

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
15. **Thumbnails are WebP** (libwebp, lossy q78). Settled by measurement in M0 — AVIF
    encoded 41x slower and would not decode at all in the `image` crate on this
    platform, for a 12% size win. See [docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md).
16. **WebView2's data directory must be redirected into the app directory.** Tauri
    defaults it to `%LOCALAPPDATA%\<bundle-id>\`, which silently breaks rule 11. Found
    in M0; must be configured before anything else ships.
17. **Animated GIFs are video; nothing is converted at import.** GIF, WebP and APNG stay
    in their original format on disk. Converting to MP4 is a compression preset in M6,
    reviewed like any other. Import never rewrites an original.
18. **There is no polish phase.** M2.5 designs the interface from scratch and sets the
    standard; every milestone after it ships at that standard. Deferred polish is
    abandoned polish. [docs/mockup.html](docs/mockup.html) is an early reference drawn
    before much of the scope existed — input to M2.5, not a specification it must follow.
19. **Renaming is a property of indexing, not a one-time event.** Files the app creates
    are born `<uuid>.<ext>`. Files arriving from outside are renamed as part of being
    indexed, silently and journalled. The first-import wizard is the same operation run
    over a whole pre-existing library at once, with a dry run and a backup gate because
    the scale makes it dangerous — it is offered while opening an unimported library and
    disappears afterwards. It is never a standing button.
20. **Anything that adds a query path is verified against a synthetic library at scale,
    not just the test folder.** The working library is a few hundred files and will stay
    that way for a while, so nothing will feel slow during development. A query that is
    instant over 198 rows can be catastrophic over 100k with joins — and the effective-tag
    cache, search, and duplicate grouping are all exactly that shape. Keep a generator
    that can produce a synthetic library of 100k items and run the milestone's new queries
    against it before calling the milestone done. Scale problems are not ordinary bugs;
    they surface as architecture, and finding one after four dependent milestones is the
    expensive way.

21. **The app ships with no domain vocabulary.** No seeded archetypes, no named field
    types, no folder-name conventions, nothing that assumes what the library is *of*.
    "Person", "instagram", "Place", "Event" and every example in these documents are
    illustrations of how someone might use the app — they are never strings in the
    product. Archetypes, labels, flags and status values are created by the user, starting
    from empty.

    This is the rule that was broken twice: a migration that seeded a Person archetype with
    social-platform fields, and a "parse folder names" action built around one specific
    naming habit. Both are the same mistake — one user's current data shape promoted into
    product behaviour. When a feature only makes sense if you already know what the user
    collects, it does not belong in the app.

22. **Every noun needs a full lifecycle, written down as operations.** If the specs describe
    something the user can have — a folder, a tag, an archetype, a saved search, a status
    value — they must also describe creating, renaming, and removing it, as capabilities in
    their own right. Describing an operation only as an entry in some future context menu
    is how folder creation went missing for nine milestones: the menu item was specced
    twice and the capability never once.

## Non-goals

Deliberately excluded. Do not build these without an explicit decision to reverse:

- **Folder-name parsing.** A migration action that split directory names like
  `Name (@handle)` into a title and typed fields. Removed: the library is built inside the
  app rather than imported from an existing convention, and the feature only made sense if
  you already knew what the folders were named after. Violates decision 21.
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
      thumbs/ab/cd/<uuid>.webp
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

**Outcome:** the layout architecture is validated — first paint, relayout and
scrubber-jump latency all pass with wide margins. Two defects were located and are **M1's
responsibility to design around, not rediscover**: tile mount/unmount churn triggers GC
pauses that make fling fail its target, and scrubber drag repaints per jump. Both are
specified in [docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md).

### M1 — Core library (strictly read-only)

Root picker, filesystem walk, BLAKE3 hashing, metadata extraction, thumbnail and sprite
generation, persistent job queue, folder tree, the real grid.

**M1 must not modify a single file inside the library root.** It reads, indexes, and
writes only into `.gallery/`. Files keep whatever names they already have — content hash
is identity, so the rename can happen later and everything re-links by hash. This means
the first version can be pointed at a real 300GB library with zero risk, and any bug
found in indexing costs nothing.

### M1.5 — First-import wizard

The UUID rename. Specified in [docs/DESIGN.md](docs/DESIGN.md#first-import): scan, dry
run, backup acknowledgement, batched execution writing the reversal map continuously,
then verification.

**Scoped to the rename only.** Parsing existing folder names into archetype fields is
part of the same wizard in DESIGN.md, but archetypes do not exist until M2 — that step
lands there instead.

**Why here and not later.** The library is currently a small disposable test folder, so
this is the safest moment the rename will ever have: it gets exercised and debugged on
files that can simply be re-copied. Deferring it means the first real execution happens
against an irreplaceable collection, and it means every milestone in between is built
against M1's `disk_name` fallback rather than the actual filename model.

Build the reversal script **before** the rename runs for real. Assume it will be needed.

### M1.6 — Wizard placement, and rename on arrival

Two things M1.5 left in the wrong shape:

- The wizard is a permanent "First import" button. It should be a step in opening a
  library that has never been imported — detected from the absence of an `imported_at`
  marker plus non-UUID filenames — and then gone from the interface. Settings keeps a
  **Normalise filenames** action for the repair case.
- Nothing renames files that arrive *after* the import. Pull the wizard's per-file rename
  out of `fs/import.rs` so the indexer and watcher use it too: anything entering an
  imported library gets a UUID name as part of being indexed, silently, journalled, with
  `orig_name` preserved. Files the app writes itself are born UUID-named and skip this
  path entirely.

Specified in [docs/DESIGN.md](docs/DESIGN.md#first-import) under *After the first import*.

Also fix the **blank grid in dev mode** found during M1.5 — an asset-protocol quirk that
does not affect release builds. Worth clearing now rather than living with it: every
milestone from here uses the dev loop, and a dev mode that cannot show the grid pushes
sessions toward release-only testing, which is slow enough to discourage testing at all.

### M1.7 — Import as a startup flow

M1.5 and M1.6 built the right operation behind the wrong interface. Rewrite it against the
revised [docs/DESIGN.md](docs/DESIGN.md#first-import) §10:

- **Full-window screens, not a modal.** Choose folder → Review → Progress → Gallery, in
  the picker's visual language. The import currently renders over a gallery that is already
  indexing and generating thumbnails for files it is about to rename.
- **Nothing is written before the rename.** No indexing, no thumbnails, no `.gallery/`
  content until the library has been normalised. The order becomes rename, then index.
- **Two screens, one checkbox.** Scan, dry run, backup prompt, execute and verify collapse
  into a single Review screen — counts, a five-row before/after sample, one backup
  acknowledgement — plus a Progress screen. Verification runs silently and surfaces only on
  failure.
- **Cancel returns to the folder picker.** There is no read-only half-state to maintain.
- **Delete the reversal tooling.** `src/bin/reverse_import.rs` goes. Original filenames are
  metadata in `item.orig_name`, shown in each file's details and searchable. The
  uuid-to-original mapping stays in `library.jsonl` as part of the disaster-recovery export,
  but reconstructing names is not a feature.

Removing the reversal is what earns the shorter flow: with no undo, the backup
acknowledgement is the one interruption that carries weight, so everything else can go.

### M1.8 — The library is live

Remove the **Re-index** button and replace it with a filesystem watcher, per
[docs/DESIGN.md](docs/DESIGN.md) §10 *The library is live*. Indexing stops being something
the user asks for.

`fs/watch.rs`, built on the `notify` crate over Windows' `ReadDirectoryChangesW`. This is
an OS-level notification API on a single recursive directory handle — **no polling, no
per-file cost**, which is why the preferred design is achievable rather than a compromise.

The parts that need care, none of which are the watching itself:

- **Settling.** A file copied in from Explorer emits events long before it is complete.
  Wait for size and mtime to stop changing before hashing. Indexing a half-copied file
  records a hash for something that will not exist a second later.
- **Self-suppression.** The app renames files and writes into `.gallery/`. Exclude
  `.gallery/` from the watch, and suppress paths the app is mid-write on, or the watcher
  feeds its own work back to itself.
- **Overflow.** Windows drops notifications when too many arrive at once and reports the
  overflow. On overflow or watcher error, run a full reconcile walk and say so in the
  readout. Silent divergence between disk and database is the only unacceptable outcome.
- **Modification keeps identity.** A changed file has a new content hash but is the same
  item. Update in place, anchored on path; do not create a second row.
- **Progress is a transient readout**, not a panel. *Indexing 42 items…*, self-dismissing.

### M1.1 — M1 defects

Small, and worth clearing before M2 builds on top:

- **Items fail to index with no explanation.** Find out why, per item, and either handle the
  cause or report it in a way that says which files and what went wrong. Thumbnails
  otherwise render correctly — this is about the failures specifically.
- Failure count and retry affordance persist after a re-index that had no failures.
- Two scrollbars. Hide the native one; the scrubber is the affordance.
- The WebView default context menu appears on right-click. Suppress it.
- No way to change the library folder once chosen.

### M2 — Folders as entities

Folder records, titles, archetypes, labels, flags, favorites, folder status, notes, tag
inheritance and the materialised effective-tag cache. Enough UI to exercise all of it —
editable fields in the folder header, tags in the existing panel — but the visual pass is
M2.5.

**Decision 20 applies here more than anywhere.** The effective-tag cache materialises
roughly ten rows per item; at 100k items that is a million rows, rebuilt on every folder
move and tag edit. Verify it against a synthetic library at scale before calling this
done. This is the milestone where a scale problem would be most expensive to find late,
because everything from M3 onward queries through it.

Four scope decisions, settled:

- **Archetypes are seed-and-apply.** Migration seeds Person, Place and Event; the folder
  header gets a picker that applies one. No archetype editor — folders carry labels
  independently, so anything an archetype lacks is added as a one-off label on the folder.
  The editor is M2.5's to design.
- **Folder-name parsing is a Settings batch action**, shaped like Normalise filenames: scan
  every folder for `Name (@handle)`, show an editable table, apply on confirm. Only touch
  folders whose title still equals the raw directory name, so it is idempotent and cannot
  clobber a manual edit.
- **A minimal details panel is in scope, as disposable scaffolding.** Fixed width, no media
  preview, no resizing, no styling investment — a tag list showing inherited greyed and
  manual solid, with add/remove. Without it the effective-tag cache is never observed
  working. M2.5 deletes it.
- **The scale check is DB-only and permanent.** A test helper that inserts synthetic
  folders, items and tags into a scratch database — no files on disk, since M2 adds no IO
  or decode paths. Measure the invalidation paths, not just the initial build: a root-level
  folder tag edit, a folder move with many descendants, the folder tree with recursive
  counts, and a tag-filtered item query.

  Shipped as the `synth_library` binary, which is good for ad-hoc runs at any size. It
  needs a companion **`#[ignore]`d test** calling the same generator at a fixed size and
  asserting the budgets, so `cargo test -- --ignored` stays the single standing gate
  alongside `scale_check_100k_items`. A check nobody remembers to run is not a check.

**The effective-tag rebuild is a job, never a synchronous command.** A tag edit on a
top-level folder invalidates the whole library. Inside a `#[tauri::command]` that freezes
the window — see [docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md). Build it as a job
now; retrofitting once M3 onward queries through it is far worse.

Also carries three small items deferred from earlier milestones:

- **Rename the binary.** `tauri.conf.json` has `productName: "gallery"` and `Cargo.toml`
  the package name `gallery`, so the build produces `gallery.exe` while the repo, the
  window title and the README all say GGallery. The identifier `local.ggallery` is already
  correct. Check the WebView2 data directory path still resolves after the change.

- **Folder-name parsing**, held over from M1.5 because it needs archetypes. Existing
  folders named `Ana (@ana)` are offered as `title: Ana` with `instagram: @ana` on the
  Person archetype, as an editable table before anything is applied.
- **Animated GIF classification.** `media/mod.rs` classifies every GIF as `kind = image`;
  locked decision 17 requires animated GIF, WebP and APNG to index as `kind = video`.

### M2.1 — Operations, and a vocabulary the user owns

Two failures found together, both fixed here.

**The app cannot change the thing it organises.** No way to create a folder, rename one,
move one, delete one, or move items between them. Folder creation appears in the specs
only inside the sidebar's right-click menu (M2.5) and the triage flow (M4) — described
twice as a menu item, never once as a capability. Decision 22 now forbids that shape.

**The app ships someone's domain.** The migration seeds `Person`, `Place` and `Event`
archetypes with `instagram`, `tiktok`, `youtube` and `twitter` fields, and Settings offers
a "parse folder names" action built around one naming habit. Decision 21 now forbids that
too.

#### Remove

- **Folder-name parsing, entirely** — modal, command, backend, docs. It is a migration tool
  for a library that will be built inside the app rather than imported.
- **Seeded archetypes and their fields** from `002_folder_metadata.sql`. Ship none.
  Existing test libraries need a migration that drops them.
- **Platform knowledge from the `handle` field type.** It becomes text matched with or
  without a leading `@` — no auto-linking, no platform. Links use `url`.

#### Add, because removing the seeds requires it

- **Archetype management** in Settings: create, rename, delete, and add, reorder or remove
  typed fields, with the "N folders use this — add the field to them?" prompt on edit and a
  named confirmation before removing a field that holds values. With nothing seeded, an
  editor is no longer optional.
- **Folder status management**: rename, recolour, reorder, add and remove status values.

#### Folder and item operations

Specified in [docs/DESIGN.md](docs/DESIGN.md) §1 *Folder operations*, *Item operations* and
*Selection*:

- **Create** a folder — directory plus record, optional archetype.
- **Rename** — title and directory name are independent. Retitling touches the record
  only; renaming the directory moves it on disk and rewrites every descendant `rel_path`.
- **Move** a folder — descendants follow, and the effective-tag cache rebuilds for the
  subtree because inherited tags are recomputed from the new ancestry.
- **Move items** between folders — real file move, `folder_id` update, tag-cache rebuild.
- **Delete** to `.gallery/trash/` with relative paths preserved. Never a hard delete; this
  pulls `fs/trash.rs` forward from M4.
- **Delete items** from the grid, not only from triage and theatre view.
- **Reveal in Explorer** and **open with the default application** — the escape hatches an
  app that renames everything to a UUID owes the user.
- **Copy the file** to the clipboard via `clipboard-win` and `CF_HDROP`, so `Ctrl+V`
  pastes a real file rather than a string. Copy-path stays a separate action. The file goes
  on the clipboard under its UUID name; staging a copy under a reconstructed name waits for
  M8, where that logic already has to exist for Export.
- **Select all, invert, clear**, each bound, with a live selection count.
- **Rename and delete a tag.** Tags are created inline from M2 onward, so without this a
  typo is permanent until M8's management screen. The full screen — merge, aliases, usage
  counts — stays in M8; this is the minimum that stops the vocabulary rotting.

Everything here is **journalled**, so M4's replayer covers it retroactively. Path rewrites
across a large subtree are a job, not a synchronous command — same reasoning as the
effective-tag rebuild.

UI is disposable scaffolding again: whatever is cheapest to exercise the operations. M2.5
designs where these controls actually live.

### M2.2 — One folder name

M2.1 made the display title and the directory name independently editable. That was my
call and it was wrong: two operations behind what reads as one control, with a silent
failure mode — a folder keeps whatever name it was born with unless you separately
remember to rename the directory.

Collapse them. Retitling renames the directory, derived from the title and sanitised for
Windows; the subtree-rename job M2.1 already built does the work, and at 105ms over 100
descendants the churn that motivated the split is not worth avoiding.

Specified in [docs/DESIGN.md](docs/DESIGN.md) §1 *Folder names*: forbidden characters,
reserved device names, trailing dots and spaces, a length cap, sibling collisions, and
what happens when a title sanitises to nothing. Renaming a directory in Explorer updates
the title, unless the title already sanitises to that name.

**Do this before M2.5**, small as it is — otherwise the redesign ships two rename controls
that immediately collapse into one.

### M2.5 — The interface, designed from scratch

**This is a design milestone, not a restyling job.** It does not improve the existing
interface, extend it, or bring it up to a standard. It asks what the best end-user
interface for an application of this scope would be, and builds that.

The current UI and [docs/mockup.html](docs/mockup.html) are **inputs, not constraints**.
The mockup was drawn early, before much of the scope existed; it is one proposal among
several and may be rejected in whole or in part. Concluding that the sidebar belongs
elsewhere, that the query bar should work differently, or that the whole layout model is
wrong are all legitimate outcomes.

**This is the milestone that decides whether GGallery is a good viewer.** The grid, the
preview, theatre view and how you move between them are the product — not chrome around an
organiser. Design them first and let the organisational surfaces arrange themselves around
that, rather than the reverse.

#### Requirements the design must satisfy

Almost everything is open. These are not:

- **The library root is not a folder.** *Everything*, *loose items at the top level*, and
  *the folder tree* are three distinct things, and an empty tree renders as empty rather
  than as a lone root node. [docs/DESIGN.md](docs/DESIGN.md) §2 *Navigation roots*.
- **Moving items and folders must work by direct manipulation.** The workflow being
  replaced is dragging between two Explorer windows; requiring a context menu for every
  move is a regression. The gesture is open, its existence is not. §2 *Direct
  manipulation*.

#### Phase 1 — Design. Nothing is built.

1. Read the full scope in [docs/DESIGN.md](docs/DESIGN.md): what this app is for, the
   workflows it exists to replace, everything from folders and tags through triage,
   downloads, compression review and multi-view.
2. Look at how comparable applications solve these problems — media libraries, DAM tools,
   photo managers, file browsers — and what they get right and wrong.
3. Produce **two or three genuinely different directions**, not variations on one. Show
   them concretely enough to react to.
4. Present, then converse.

#### Interaction rules for phase 1

This phase is deliberately conversational, and how much is asked matters:

- **Where two options are genuinely comparable** and the choice is taste, ask. Present the
  tradeoff honestly rather than steering.
- **Where one option is clearly better**, say so, explain why, and confirm before adopting
  it. Recommend — do not silently decide.
- **Where a convention obviously applies**, decide quietly. Do not ask which side the
  close button goes on.
- **Never assume.** No blind choices on anything that shapes how the app is used.

#### Phase 2 — Build. Only after the design is approved.

**Set up frontend testing here**, not earlier. `vitest` plus `@testing-library/react` with
the IPC layer mocked, covering interaction rather than appearance: does picking an
archetype call the right command, does editing a label persist, does adding a flag update
the tag set. That is exactly the class of bug M2 hit — an archetype dropdown that focused
the notes field instead of registering the selection — and it is invisible to Rust tests
and to `tsc`.

It waits for M2.5 because this milestone replaces the interface wholesale; tests written
against M2's throwaway UI would be thrown away with it. From M2.5 onward the UI is stable
enough to be worth testing, and every later milestone inherits the harness.

Then implement it, including the surfaces waiting on this milestone: the preview panel,
theatre view with left/right navigation and a filmstrip, resizable panels, and right-click
menus for folder, item, selection and empty space.

Separated from M2 deliberately. Folded together, the data work eats the schedule and the
interface arrives as an afterthought. Given its own milestone, it also establishes the
component vocabulary every later milestone builds from.

### M3 — Search

Query parser, unified search bar, sectioned results (folders then media), saved
searches, FTS index.

### M4 — Sorting Box and triage

Fullscreen culler with bindable destination hotkeys, grid multi-select mode, inline
folder creation, drag-and-drop from Windows, filesystem watcher, undo journal, trash.

Removes the most tedious chore the app replaces — two Explorer windows and a lot of
dragging. One job among several, though: the viewing experience is the product, and M2.5
is where that is decided.

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

### M10 — Multi-view

Up to twelve items in theatre view at once, all playing, adaptive layout, one audio solo.
Specified in [docs/DESIGN.md](docs/DESIGN.md).

**Starts with a measurement, not a build.** Find out how many concurrent video streams
actually hold frame rate on the target machine before committing to twelve — hardware
decode sessions are finite and the fallback to software decode is silent. If the real
number is six, the cap is six.

Separated from M2 deliberately: the theatre view itself is cheap, this is not, and the
risk belongs where it can be abandoned without taking the viewer down with it. Can be
pulled forward once M4 lands if it turns out to be wanted sooner.
