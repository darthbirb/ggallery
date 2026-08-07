# Roadmap

One milestone at a time, in order. Do not start the next one, and do not pull features
back from it because they seem convenient.

**Shipped milestones are one line.** They describe what the app does now, not how it got
there — a milestone that added a control and a later one that moved it read as one line
saying where the control is. Anything from a finished milestone that is still worth
knowing lives in [DECISIONS.md](DECISIONS.md) or [NOTES.md](NOTES.md), not here.

---

## Done

| | |
| --- | --- |
| **M0** | Grid performance spike. Architecture validated, two defects found. Numbers in [NOTES.md](NOTES.md). |
| **M1** | Core library — index, hash, thumbnails, job queue, grid. Read-only. |
| **M1.5–M1.7** | First import as a startup flow: choose a folder, review, rename every file to a uuid, index. Two screens, gated on a backup acknowledgement, no reversal tooling. |
| **M1.8** | The library is live. A filesystem watcher picks up changes on disk with no re-index button. |
| **M2** | Folders as entities — archetypes, labels, tag inheritance. |
| **M2.1** | Folder and item operations — create, rename, move, delete, all journalled and undoable from the toast they raise. |
| **M2.5a** | The shell and the viewer — split layout, navigation panel, folder band, pane in Preview mode, accent system, toast-and-undo, complete right-click menus. |
| **M2.5c** | The shell decided — our own window bar, the mark, the pinned navigation footer, the reworked folder band. |
| **M2.5d** | Lowercase vocabulary, cursor-anchored zoom, the grid footer's selection count, folder ancestry breadcrumbs. |
| **M2.6** | Folders as data — flat sharded storage under `files/`, hierarchy in the database, watched `inbox/`, `library.jsonl` as the rebuild path. |
| **M2.6a** | First import mirrors the directory tree into folder records instead of flattening it. |
| **M2.5b** | The sorting surfaces — the pane's Grid and Folders modes, three drop targets, spring-loading, inline folder creation. |
| **M2.8a** | The drawing reconciled against the specification. Findings in [design/DEVIATIONS.md](design/DEVIATIONS.md). |
| **M2.8b** | The drawing's colour, type and height scales taken into the token layer; primitives restyled; Title Case across the chrome. |

## Now

### M2.8c — the surfaces

Building the drawing's layout, in three passes. Everything not yet built is listed in
[design/DEVIATIONS.md](design/DEVIATIONS.md); that file is the checklist.

**M2.8c.1 — the shell and the tile.** The frame everything else sits in, so it goes first.
Window bar to 36px, navigation panel to 232px, library totals in the footer, a *New Root
Folder* button, hover actions on tree rows. Plus the tile's four corners — selection wash
and tick, favourite, video duration, format label — which are new nodes in `TilePool` and
therefore need re-measuring against 100k per decision 20.

**M2.8c.2 — the band and the pane.** The folder band's full control strip, labels and tags
on separate rows in both the band and the details panel, the filmstrip, the pane headers.
Brings **sort** (captured date, added date, size, duration, random) and the **uniform grid
layout**, both of which touch the backend.

**M2.8c.3 — the rest.** Settings, menus, dialogs, toasts, the import screens, the failure
banner as far as its current data reaches.

### M2.8d — the drawn-ahead screens

The drawing covers Search, Triage, Downloads, Pending Review, Duplicates, Storage, Tags and
Multi-View. None of them are built. This pass writes each into [../SPEC.md](../SPEC.md) as
behaviour, so the milestone that owns it builds from a specification with a picture behind
it rather than from the picture alone.

### M2.9 — the nitpick pass

The user goes over the whole interface in use and complains about everything; then it gets
fixed. The list lives in [NITPICKS.md](NITPICKS.md), which is open from now on — an
annoyance noticed mid-milestone goes in the file rather than into whatever session is
running.

**Every item is asked whether it is an instance or a class.** *"The fill icon points
nowhere"* is one control; *"an icon names the action, not the state"* is the rule underneath
it, and the rule is what stops the next one. Items that turn out to be classes are written
into [DECISIONS.md](DECISIONS.md) or [../SPEC.md](../SPEC.md), not just fixed.

Three outcomes, all legitimate: **fix**, **change the spec**, or **won't do, and here is
why** — recorded, so it is not re-raised in six months.

## Next

### M3 — Search

Query parser, unified search bar, sectioned results (folders then media), saved searches,
FTS index. **Decision 32 lands here** — every chip becomes a query term in one pass, across
the breadcrumb, the folder band and the details panel, because a chip needs a bar to write
into.

### M4 — Sorting Box and triage

Fullscreen culler with bindable destination hotkeys, grid multi-select mode, Explorer
drag-and-drop, trash. Removes the most tedious chore the app replaces.

Two things to settle here: the drawing makes the Sorting Box **a screen with its own
header** where the build makes it a scope of the ordinary grid, and it puts a standing
*Send to* bar on that screen. Both are structural, not appearance.

### M5 — Downloads

URL → tool detection → download into the Sorting Box, auto-labelling with source and
uploader, archive-file integration so nothing downloads twice, queue view, tool updater,
cookie support (Instagram will not work without it).

### M6 — Compression and review

Preset management, HandBrakeCLI and image encoding jobs, Pending Review queue, lineage,
trash integration. **Comparison renders into the pane**, not a screen of its own — split
Preview with synced pan, zoom and timeline.

### M7 — Duplicates

Perceptual hashing, grouping, side-by-side comparison, tag merging from loser to keeper.
Uses the same split pane as M6.

### M8 — Utility screens

Storage dashboard, tag management (rename, merge, aliases, unused), export with
reconstructed filenames, integrity check.

**Indexing failures belong here.** The drawing's failure banner classifies each failure as
retryable, damaged or skipped, offers per-file retry, remediation text, an ignore list and
an export. Today the stored record carries far less and retry is all-or-nothing.

### M9 — Polish

Command palette, keyboard reference, blur toggle, `library.jsonl` export and rebuild, backup
verification.

**Interface scaling — `Ctrl` `+` / `Ctrl` `-`, plus a Settings option.** One factor over the
whole interface, media excluded. This is the right answer to "this is too small on my
monitor", because the answer differs per display and per person.

### M10 — Multi-view

Up to twelve items in theatre view at once, all playing, adaptive layout, one audio solo.
**Lands inside the pane** — multi-view is Preview mode with more panes, the same control M6
and M7 use.

**Starts with a measurement, not a build.** Find out how many concurrent video streams hold
frame rate on the target machine before committing to twelve; hardware decode sessions are
finite and the fallback to software decode is silent. If the real number is six, the cap is
six.

## Deliberately excluded

Do not build these without an explicit decision to reverse.

- **Folder-name parsing** — splitting directory names like `Name (@handle)` into a title and
  typed fields. Violates decision 21: it only makes sense if you already know what the
  folders are named after.
- **Masonry grid layout.** Column-major, so it has no rows, and the grid's windowing and
  tile recycling are built on rows. It would mean a second layout model, a second windowing
  path and a second recycler through the one piece measured against a 100k library. Revisit
  only with a real cost estimate.
- **Subscriptions** — nothing checks a source on a schedule. Downloads are manual.
- **Collections / albums** — ordered curated sets spanning folders. Saved searches plus
  favourites cover the need.
- **Star ratings and colour labels.** Favourite is binary.
- **Hidden or PIN-protected folders.** A blur toggle is the only privacy affordance.
- **Face recognition.** Possible far-future; nothing in the model should block it, nothing
  should assume it.
- **Any network service, sync or sharing.** Local, single-user.
