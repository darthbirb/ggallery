# Gallery — Plan

A gallery viewer and collection organiser for Windows, in that order.

**The main activity is looking at things.** Browsing the grid, opening an item, moving
through a folder, comparing two shots. Everything else exists to serve that: folders and
tags so you can find something again, search so you can find it faster, downloads so there
is more to look at, compression and duplicate detection so the collection stays worth
keeping, triage so filing does not become a chore that stops you adding to it.

A better media viewer, built for one person, that happens to also organise. Not an
organiser that happens to display files.

Folders are data, not directories; tags, labels and search live on top. One root
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
| Styling | Tailwind + `shadcn/ui` | Radix behaviour plus designed defaults, copied into the repo and restyled — see decision 18 |
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
6. **The Sorting Box is "no folder", not a place.** An item with no `folder_id` is
   unfiled by definition; there is no `Sorting Box/` directory and no magic location.
   Files arrive via the app, Windows drag-and-drop, downloads, or being dropped into
   the watched `<root>/inbox/`, and everything that arrives without a destination
   lands there. *(Revised twice: this decision first named a watched
   `<root>/Sorting Box/` subfolder, then the library root itself. Rule 30 made both
   moot — with files stored flat, "loose at the top level" no longer describes
   anything.)*
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
    abandoned polish. *(`docs/mockup.html` was an early drawing used as input to M2.5 and
    is now superseded by the built interface. Do not treat it as a reference.)*

    **`shadcn/ui` is the component source.** Not Radix alone — Radix is headless and ships
    no visual design at all, which is how M2.5a ended up with correct behaviour and
    hand-rolled appearance. shadcn is Radix plus designed Tailwind defaults, copied into
    the repo so they can be restyled. A bespoke layout does not require bespoke buttons.

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

23. **Nothing is keyboard-only.** Every action has a visible control. Keys are a second
    path to something already on screen, never the only path. If an action can only be
    performed by knowing a shortcut, it is not finished.

    This governs every milestone, not just the interface one. Two consequences already
    identified: `Ctrl+Z` alone is not a path to undo, so **every destructive action ends in
    a toast naming what happened with an Undo button** — which also makes the journal
    discoverable, which it currently is not. And triage, specced in
    [docs/DESIGN.md](docs/DESIGN.md) §4 almost entirely in keystrokes, needs its mouse path:
    the ordinary window, Sorting Box in the grid, folder pane open. Hotkeys stay; they stop
    being the only route.

    Right-click menus must be complete, not a subset.

24. **One accent, chosen from a fixed set.** Exactly one hue carries selection, focus,
    the active tab, drop acceptance and the panel drag handles. The user picks it from a
    short list — Slate (default), Teal, Violet, Rose, Moss, Amber — so every value can be
    contrast-checked against the same greys rather than trusting a free colour picker.

    *(M2.5a.1: **scrubber position** left this list and the drag handles joined it.* The
    scrubber sits directly against the pane's handle, and two accent bars a pixel apart
    read as one. Of the two, the handle is what you reach for and the scrubber thumb is
    what you glance at — and every other scrollbar in the app already says position in
    plain grey, so the scrubber saying it the same way costs nothing.)

    Green and red stay reserved for meaning — kept, saved, deleted, failed — and are never
    the accent. Swap `--color-accent` / `--color-accent-d` via a `data-accent` attribute on
    the root; `--color-good` and `--color-danger` are fixed. `--color-info` is deleted; it
    collides with the default accent.

25. **Controls are sized to be hit and seen.** Decided once, applied everywhere:

    - Control heights `28 / 32 / 38px` for small, default, large. Icon buttons are never
      smaller than `32×32`, whatever the glyph.
    - The glyph fills roughly 55–60% of its button — an 18px icon in a 32px button, not 12px.
    - **Every button has a visible surface at rest**: background and border. Ghost buttons
      that only materialise on hover are how a control becomes invisible until you already
      know it is there.
    - Base UI text `14px`, mono `12px`. Both a step up from where M2.5a landed.
    - **Anything clickable shows `cursor: pointer`.** *(Added in M2.5a.2 — nothing in the
      app did.* A control that keeps the arrow cursor reads as decoration, and the browser
      default for a `<div>` or a `<button>` is not a pointer. This includes rows, tiles,
      tabs, chips, swatches and drag handles — handles get the resize cursor for their
      axis, which is the same rule. **Scrollbars and the scrubber are the exception**: they
      are dragged, not clicked, and no scrollbar anywhere shows a pointer. Implement it as
      one global rule on `<button>` rather than a class per call site, or it will be as
      complete as the last person's memory of it.)

26. **One visual state for selection; focus rings are for keyboards.** Selection is a filled
    accent border. The shift-click anchor is **not rendered** — it is invisible bookkeeping,
    and drawing it competes with selection for the same meaning, which is why inverting a
    selection currently looks ambiguous. Keyboard focus uses `:focus-visible` only, so it
    never appears after a mouse click. Two outlines fighting over one tile is a design bug,
    not a styling one.

    *(M2.5a.2 splits this by shape.* A border is right on a **tile**, where the media fills
    the frame and there is no background to tint. It is wrong on a **row** — a list has no
    frame, so the same treatment draws a box around text. Rows and other list-like controls
    are selected by a **filled rounded surface**: accent-tinted background, accent text, the
    same rounded rectangle the pane header's details control uses. Hover is the same shape
    in neutral, one step lighter than the rest, so hovering a selected row is visibly not
    the same as selecting it. One shape, two intensities, and the accent only on the real
    state.)

27. **Motion is short, functional and interruptible.** Anything that changes size or
    position animates — panel folds, band expansion, details opening, filmstrip resize —
    because a panel that teleports makes you re-find your place. Nothing decorative
    animates: no entrance effects, no staggered lists, no spring overshoot.

    One scale: `120ms` for hover and colour, `180ms` for layout, `ease-out` for both.
    Anything longer is felt as lag on a control you use hundreds of times an hour. Animate
    `transform` and `opacity`; animating `height` or `width` on a surface containing the
    grid is a per-frame relayout and will cost more than the animation is worth. Honour
    `prefers-reduced-motion`.

28. **Every band owns one job, and the window bar is ours.** Horizontal chrome accretes:
    the first shell grew a bar holding the library name, its full path, index status, a
    scope checkbox, the tile-size slider, an *Open pane* button and a hamburger — seven
    unrelated things under a Windows title bar repeating the app's name. The symptom is
    that "where does this control go" has no answer, so it goes there.

    Three bands, each with an owner: the **window bar** (the window and the app — mark,
    name, window controls, later search), the **folder band** (the current grid, including
    tile size and scope), and the **navigation footer** (the app's own state — Settings,
    background work). Native decorations are off. A control that fits none of the three is
    a sign the control is wrong, not that the bar needs another slot.

    **A panel's reopen control lives on the panel's own edge**, never in a bar. The
    navigation panel folds to a 44px icon rail; the pane folds to a strip of its three mode
    icons. Symmetric, discoverable, and it keeps the window bar clean.

29. **The app has a mark, and the mark is not the accent.** GGallery ships an identity that
    reads at 16–20px in the window bar and doubles as the Windows `.ico`. It stays neutral:
    the accent is user-chosen and changes per session, and an identity that changes colour
    with a preference is not an identity.
30. **Folders are data. Files are stored flat.** *(Reverses the founding rule that the
    filesystem was authoritative; see M2.6.)* The hierarchy — parentage, titles, order —
    lives in the database and nowhere else. On disk every file sits at
    `<root>/files/<first two hex chars of uuid>/<uuid>.<ext>`, sharded 256 ways so no
    directory holds 100k entries.

    The rule it replaces existed because directory names were the last human-readable
    structure on disk. But filenames are already opaque UUIDs (rule 5), so that structure
    was carrying the entire organisation on its own, and paying for it: a folder move was
    a physical move of every file beneath it, a rename rewrote every descendant path, and
    the database and the filesystem could drift apart mid-operation and leave a record
    pointing at a directory that no longer exists. Undo had to reverse thousands of file
    operations. Titles were constrained by `MAX_PATH`, by forbidden characters, by
    reserved device names, and by case-insensitive sibling collisions.

    As data, a move is `UPDATE folder SET parent_id`, a rename is one column, undo is one
    row, and none of those constraints reach the title at all.

    **What this gives up is the redundant copy.** A lost database used to leave the
    directory tree behind as a readable record of the organisation. Two things buy that
    back, and they are load-bearing rather than nice to have: `library.jsonl` becomes the
    rebuild path rather than a convenience, and `.gallery/backups/` keeps rolling copies
    of the database. This is a net improvement, because a corrupt database today loses
    every tag while the folders survive — the tree was never a backup of the *whole*
    organisation, only of its skeleton.

    **New files arrive through `<root>/inbox/`**, which is watched. Dropping a file into
    `files/` by hand is meaningless now, so the gesture "put things in the library folder
    from Explorer" needs somewhere real to land. Anything in `inbox/` is renamed, sharded
    and recorded, and appears in the Sorting Box.
31. **Everything the tag system stores is lowercase.** Folder titles, label keys, label
    values and flags are lowercased on the way in — typing `Beach` stores and displays
    `beach`. Notes, original filenames and every other free-text field are untouched.

    Case-insensitive *matching* was already true; what was not true was case-insensitive
    *identity*, so `Beach` and `beach` could both exist and mean the same thing while
    counting separately. Since a folder title is inherited as a tag, leaving titles
    cased would keep exactly that inconsistency alive at the top of the tree.
32. **Every chip is a query term.** Clicking a folder, label or tag chip anywhere —
    the breadcrumb, the folder band, the details panel — writes its term into the search
    bar and shows the result. `path:people/ana`, `instagram:@ana`, `beach`. Ctrl-click
    adds a term to what is already there rather than replacing it.

    This is the sidebar's existing rule (*"sidebar interactions mutate this string rather
    than bypassing it"*, [docs/DATA-MODEL.md](docs/DATA-MODEL.md#query-language)) applied
    to every other clickable piece of vocabulary. One model, the bar always shows why the
    grid holds what it holds, and going back is clearing it.

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

**Absolute paths must never reach the database.** Since rule 30 there is very little
path left to store — an item's location is derived from its own uuid — but the rule
still governs the config file, the cache and the trash. This is what keeps portability
alive.

## On-disk layout

```
<root>/
  .gallery/
    library.db            ← SQLite, WAL, checkpointed on exit
    library.jsonl         ← plaintext export, and the rebuild path
    backups/              ← rolling copies of library.db
    cache/
      thumbs/ab/cd/<uuid>.webp
      sprites/ab/cd/<uuid>.webp    ← 10-frame scrub strip per video
    trash/                ← soft-deleted files, flat, same sharding
    pending/              ← compressed candidates awaiting review
    lock                  ← single-instance guard
  files/
    a3/a3f2c1d4-….jpg     ← every file, sharded by the uuid's first two chars
    b7/b7e40021-….mp4
  inbox/                  ← watched; drop files here from Explorer
```

There is no folder structure on disk. `files/` is sharded 256 ways because a single
directory holding 100k entries is slow to enumerate and painful for every backup tool
that touches it; the shard is derived from the uuid, so no lookup is needed to find a
file.

Cache runs ~4–6GB at 100k items. It stays inside root so that copying the folder gives
a working library immediately rather than a 30-minute thumbnail rebuild. One setting
relocates it; it is safe to delete at any time.

`library.jsonl` is written on a debounce, one line per item keyed by UUID, carrying its
folder path, title, tags and labels, plus one record per folder. **It is the rebuild
path, not a convenience** — since rule 30 the database is the only structured copy of
the organisation, so the plaintext one has to be complete enough to reconstruct it, and
readable in Notepad when it matters. `.gallery/backups/` keeps rolling copies of the
database itself for the same reason.

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
  *(M2.2 collapsed these into one control; M2.6 removed the directory half entirely.)*
- **Move** a folder — descendants follow, and the effective-tag cache rebuilds for the
  subtree because inherited tags are recomputed from the new ancestry.
- **Move items** between folders — real file move, `folder_id` update, tag-cache rebuild.
  *(M2.6 drops the file move; the location is derived from the uuid.)*
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

### M2.2 — One folder name *(superseded by M2.6)*

M2.1 made the display title and the directory name independently editable. That was my
call and it was wrong: two operations behind what reads as one control, with a silent
failure mode — a folder keeps whatever name it was born with unless you separately
remember to rename the directory. M2.2 collapsed them, deriving the directory name from
the title and sanitising it for Windows.

**Decision 30 deleted the problem rather than solving it.** With no directories, a folder
has a title and nothing else to keep in step; the sanitising rules, the reserved device
names and the sibling-collision suffixes all leave with them. Kept here as history
because the reasoning — *two operations behind one control is a bug* — is what the
single-name rule still rests on.

### M2.5 — The interface, designed from scratch

**This was a design milestone, not a restyling job** — it asked what the best interface for
an application of this scope would be and built that, rather than improving what existed.
The grid, the preview and how you move between them are the product, not chrome around an
organiser, so they were designed first and the organisational surfaces arranged around them.

**Phase 1 is finished.** It produced three directions, four settled forks, and the layout
now specified in [docs/DESIGN.md](docs/DESIGN.md) §2 — one split with a polymorphic pane, a
resident navigation panel, a collapsed folder band, and drops onto tree rows and folder
tiles. DESIGN §2 is the specification; this section is only the build order. Two outcomes
worth keeping because their absence is deliberate: **there is no permanent destination bar**
along the bottom, the folder pane replaced it, and **the pane never shows folder
information** — the band owns that.

#### Phase 2 — Build, in two passes

Too much for one milestone, and split along the line the product cares about: **2a is "can I
look at my library properly", 2b is "can I sort it fast".**

**M2.5a — the shell and the viewer. Built.** The split layout, the navigation panel, the
folder band, the accent system, toast-and-undo, complete right-click menus, and the pane in
**Preview mode only** — built so splitting into N panes is an extension, not a rewrite.
Removed the disposable scaffolding from M2, M2.1 and M2.2 rather than layering over it.

Three corrective passes followed, each one a class of mistake rather than a list of bugs.
The outcomes are all in decisions 18 and 24–27 and in DESIGN §2; kept here only as history:

- **M2.5a.1 — make it look built.** M2.5a shipped correct structure with hand-rolled
  appearance because the brief told it to take Radix and avoid a component kit. Radix is
  headless and ships no design at all. `shadcn/ui` adopted properly, decisions 25 and 26
  applied throughout.
- **M2.5a.2 — the rest of the finish.** Motion, cursors, thicker scrollbars, the scrubber's
  date and the filmstrip's counter deleted. Three things arrived that were not in the brief
  — Settings collapsing to one dialog, maximise animating instead of unmounting, the tree
  rebuilt as recursive nesting — all the same mistake: a conventional surface left
  unspecified and therefore invented. That is what DESIGN §*Prior art* now prevents.
- **M2.5a.3 — build versus adopt.** Audited the navigation panel, Settings, the resizable
  split and the toaster against shadcn's registry. **Nothing was adopted**, and the reasons
  are recorded in [docs/ENGINEERING-NOTES.md](docs/ENGINEERING-NOTES.md) so the question is
  not re-asked. Also added the dev-only kitchen-sink route, which is how every later
  milestone reviews its own appearance in one screenshot.

**M2.5c — the shell, decided.** Runs **before** M2.5b, because 2b adds two modes to a pane
header this milestone redesigns, and building them twice is the avoidable cost. Per
decisions 28 and 29, and DESIGN §2:

- **Our own window bar.** `decorations: false`, mark and *GGallery* left, minimise /
  maximise / close right in Windows order, the rest a drag region. Snap Layouts is
  knowingly given up — see DESIGN §2 for why that is acceptable here.
- **Design the mark.** Neutral, legible at 16px, and regenerated as the `.ico`.
- **The library name and path leave the chrome entirely.** They belong in Settings.
- **Settings and job status move to a pinned navigation footer**, surviving the fold.
- **Tile size and *this folder only* move into the folder band**, which owns the grid.
- **The pane's mode buttons are always visible**, Preview shows an empty state with nothing
  selected, and a closed pane folds to a strip of its three mode icons. The *Open pane*
  button is deleted.
- **Rework the expanded folder band** to DESIGN §2 *The expanded band is identity, not a
  form* — one counts line, no `Active` chip, notes as a growing line, one chip row, the
  archetype action demoted to the folder menu, and roughly 140px when empty rather than 330.

**M2.5d — the follow-ups.** Small, independent, and none of them blocked by M2.6 below:

- **Lowercase on the way in** (decision 31) for titles, keys, values and flags, in the
  input controls. The data migration that merges existing collisions belongs to M2.6,
  where the folder tables are already being rewritten.
- **Every chip is a query term** (decision 32) — breadcrumb, folder band and details panel.
- **Zoom anchors on the cursor**, not the centre of what is visible. *(Specified as the
  centre in M2.5c and corrected in use: the centre is right when zooming with a keyboard
  and wrong when zooming with the wheel, because the wheel already tells you where to
  look.)*
- **Fill-window is an arrow, and it moves left** — pointing left to fill, right to
  restore, placed to the left of *Details* rather than among the right-hand controls. The
  icon it replaces described a state rather than the action it performs.
- **The footer's selection count moves right**, replacing *right click for more* — which
  is instruction, not status, and instruction the user needed once.
- **Folder details show their ancestry**, the same breadcrumb the item details panel
  already has, target folder included.
- **A folder whose directory is missing must fail usefully** rather than reporting *the
  system cannot find…* on every action. M2.6 removes the cause; this makes the symptom
  actionable in the meantime, and error text that names the folder and offers a way out
  is worth having whatever the storage model is.
- **Clicking an item while the pane is folded must open it**, once, and the grid's
  scrollbar must not paint over the folded strip.

**M2.6 — folders as data.** Decision 30. Runs **before** M2.5b, which builds drag-to-move
and inline folder creation — the exact operations whose meaning this changes, and which
would otherwise be built against paths and then rebuilt.

- **Schema.** `folder.rel_path` is dropped; identity is `id` and hierarchy is `parent_id`.
  `UNIQUE(parent_id, title)` replaces the path's uniqueness. `item.folder_id` becomes
  nullable — `NULL` is the Sorting Box.
- **Storage migration.** Every file moves to `files/<xx>/<uuid>.<ext>`. This is the
  single most dangerous operation the app has ever performed: it touches every file in a
  100k library, and a half-finished run must be resumable rather than ambiguous. Journal
  it, verify by hash, and write `library.jsonl` *before* moving anything so the mapping
  exists on disk independently of the database. Offer a dry run. This is M1.5's problem
  again and should reuse M1.5's shape.
- **Lowercase migration.** Fold titles, keys, values and flags to lowercase, **merging**
  the collisions this creates and reporting what it merged. `Beach` and `beach` become
  one tag carrying both sets of attachments.
- **`inbox/`**, watched, replacing the watched library root.
- **`library.jsonl` becomes the rebuild path** — complete enough to reconstruct the
  database, folders included, not just items. Plus rolling `.gallery/backups/`.
- **Repair the records the old model broke**, including folders left pointing at
  directories that no longer exist after a move.

Everything the filesystem used to enforce — forbidden characters, reserved device names,
`MAX_PATH`, sibling collisions — leaves the codebase with it. M2.2 exists only as
history after this.

**M2.5b — the sorting surfaces.** The pane's **Grid** and **Folders** modes, all three drop
targets, spring-loading, and inline folder creation in the folder pane. **Depends on M2.6**:
a drop is a row update here, not a file move.

**The mode switcher is icon buttons in the pane header**, in the same group as maximise and
close — not a labelled tab row. *(Decided in M2.5a.2, before the modes exist.* Three text
tabs are the widest thing in a header whose whole job is naming the item, and the header
already holds icon buttons that read correctly at that size.)

Neither ships until it looks finished. There is no polish phase.

#### Build notes

**`shadcn/ui` is the component source**, per decision 18 — Radix for the behaviour that is
tedious to get right by hand (context menus, dialogs, dropdowns, tooltips, sliders,
tabs, selects), plus designed Tailwind defaults copied into `src/components/ui/` and
restyled there against the app's own tokens. A bespoke layout does not require bespoke
buttons. *(This note originally said the opposite — "do not adopt a visual component kit
wholesale" — which is the brief that produced M2.5a's hand-rolled appearance and that
M2.5a.1 reversed.)*

**The frontend test harness landed here** — `vitest` plus `@testing-library/react` with the
IPC layer mocked, covering interaction rather than appearance. It waited for M2.5 because
this milestone replaced the interface wholesale; every later milestone inherits it.

**The surfaces themselves are specified in [docs/DESIGN.md](docs/DESIGN.md) §2**, not here —
the split, the pane and its three modes, the navigation panel, the folder band, the four
right-click menus, and §*Drops* for the three targets and spring-loading. This section is
build order only. Two things that section states and that get lost otherwise: **subfolders
never appear in the grid**, and **every drop ends in a toast naming the destination with an
Undo button**.

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

Preset management, HandBrakeCLI and image encoding jobs, Pending Review queue, lineage,
trash integration.

**Comparison renders into the pane**, not a screen of its own — the split Preview mode with
two panes, synced pan and zoom, and a shared timeline. Same for M7.

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

**Lands inside the pane**, not as its own screen: multi-view is the pane's Preview mode with
more panes, the same control M6 and M7 use.

**Starts with a measurement, not a build.** Find out how many concurrent video streams
actually hold frame rate on the target machine before committing to twelve — hardware
decode sessions are finite and the fallback to software decode is silent. If the real
number is six, the cap is six.

Separated from M2 deliberately: the theatre view itself is cheap, this is not, and the
risk belongs where it can be abandoned without taking the viewer down with it. Can be
pulled forward once M4 lands if it turns out to be wanted sooner.
