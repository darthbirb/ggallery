# Design

Product and UX specification. See [DATA-MODEL.md](DATA-MODEL.md) for schema and query
syntax.

---

## 1. Core concepts

### Folders

A folder is a real directory on disk *and* a record in the database. It has:

- **Title** — free text, anything. A person, a place, an event, a topic.
- **Archetype** — optional template that pre-creates a set of empty labelled fields.
- **Labels** — key/value pairs, nullable. `instagram: @ana`, `city: lisbon`.
- **Flags** — plain tags. `archived`, `favourite`.
- **Cover** — an item inside it, chosen manually or picked automatically.
- **Count** — items directly inside, and items recursively beneath.
- **Status** — `Active` / `WIP` / `Done` / `Archived`, user-editable set. Renders as a
  coloured dot in the sidebar tree and is queryable as `status:wip`.
- **Last added** — tracked automatically. Makes WIP actionable: the folder list can be
  sorted by staleness, surfacing *"Ana — WIP — nothing new in 5 months"* instead of a
  flag you set once and forgot.
- **Notes** — free text, searchable.
- **Pinned** — pinned folders float to the top of the sidebar.

Folders nest arbitrarily. Every folder contributes its title as a tag automatically —
that is not editable and not removable.

### Folder operations

Folders are created, renamed, moved and deleted from inside the app. Every one of these
changes the filesystem and the database together, and the filesystem is authoritative:

- **Create** — makes the directory on disk and the record, with an optional archetype.
- **Rename** — the display title and the directory name are separate. Changing the title
  alone touches only the record. Renaming the directory moves it on disk, and every
  descendant's `rel_path` updates with it.
- **Move** — dragging a folder onto another, or a menu action. Descendant paths and the
  effective-tag cache both follow, because inherited tags are recomputed from the new
  ancestry.
- **Delete** — the folder and its contents go to `.gallery/trash/` with their relative
  paths preserved. Never a hard delete.

Items move between folders the same way: drag onto a sidebar folder, a menu action, or a
triage hotkey. A move is a real file move plus a `folder_id` update plus a tag-cache
rebuild for that item.

**All of these are journalled** so `Ctrl+Z` reaches them once the replayer lands. Renames
of *files* remain the exception — see §10.

### Item operations

Beyond moving and tagging, the operations any file manager is expected to have, and which
this one needs because filenames on disk are opaque UUIDs:

- **Delete** the selection to `.gallery/trash/`. Available from the grid, not only from
  triage and theatre view.
- **Reveal in Explorer** — opens the containing folder with the file selected. The single
  most useful escape hatch in an app that renames everything to a UUID.
- **Open with the default application** — hand the file to whatever the OS associates with
  it, for the cases this app deliberately does not handle.
- **Copy the file** to the clipboard, so `Ctrl+V` into Explorer, a chat window or an email
  pastes the actual file. This needs Windows' native `CF_HDROP` clipboard format, which
  Tauri's clipboard plugin does not cover — `clipboard-win` does, and the Windows-only
  dependency costs nothing in a Windows-only app.

  **Known limitation, fixed with Export in M8:** the file is put on the clipboard under its
  real name on disk, which is a UUID. Pasting it elsewhere produces `a3f2c1d4.jpg`. The fix
  is to stage a copy under a reconstructed name first, and that naming logic belongs with
  Export rather than being written twice.
- **Copy the absolute path** as plain text — a separate action, for when a path is what you
  actually want.

### Selection

Click selects, shift-click extends a range, ctrl-click toggles one, drag draws a marquee.
Beyond that: **select all**, **invert selection**, and **clear selection**, each with a
keyboard binding, plus a live count of what is selected. Every item operation above acts
on the whole selection.

### Tags

Two shapes, one system:

- **Label** — has a key and a value. `instagram: @ana`. Searchable by key, by value, or
  by both.
- **Flag** — value only. `beach`, `blurry`.

Both live in the same table and are matched by the same query syntax. A label with an
empty value still exists and still renders — that is what makes archetype fields visible
while unfilled.

### Inheritance

An item's **effective tags** are:

```
  every ancestor folder's title
+ every ancestor folder's labels and flags
+ the item's own manual tags
```

For an item in `People / Ana / 2024 Trip`, that resolves to `People`, `Ana`,
`2024 Trip`, `instagram: @ana`, plus anything set directly on the item.

Inheritance is **live**. Move an item and its inherited tags are recomputed from the new
location — the old folder's tags drop off. When a move would drop tags, the move
confirmation offers a one-click "keep *Ana* on this item" that converts the inherited tag
into a manual one. That is the escape hatch for the case where a photo of Ana genuinely
belongs in `Places / Beach`.

Effective tags are materialised into a cache table for query speed, invalidated when an
item moves, a folder's tags change, a folder moves or is renamed, or an archetype is
applied. See [DATA-MODEL.md](DATA-MODEL.md#tag-resolution).

### Archetypes

A named set of field definitions, created and managed entirely by the user.

**The app ships with none.** There is no seeded archetype, no suggested name, no example
in the interface — see locked decision 21. Any archetype named in these documents is an
illustration of how someone *might* use the feature, never a string in the product.

Field types are `text`, `handle`, `url`, `date`, `number`. `handle` is text matched with or
without a leading `@`; it carries no knowledge of any platform and does not auto-link. A
field that should be a link uses `url`.

**Archetypes have a full lifecycle**, managed in Settings: create, rename, delete, and
add, reorder or remove fields.

Creating a folder offers an archetype picker, which is empty until the user has made one.
Choosing an archetype creates its labels with empty values, visible and waiting in the
folder header.

Editing an archetype prompts before touching folders that use it — *"3 folders use this
archetype. Add the new field to them?"* Removing a field never deletes existing values
without an explicit confirmation naming the affected folders.

### Folder status

The status set is user-defined too, with the same rule: the app ships with a small
unopinionated default set that can be renamed, recoloured, reordered, extended or removed
in Settings. Status names describe workflow state, not subject matter, so a default set is
safe where a default archetype is not — but it is still fully editable.

### Items

Every file is an item. Filenames on disk are `<uuidv4>.<ext>` — the app owns them
completely. The original filename is stored in the database, is searchable, and is shown
in the preview panel.

Items carry: captured date (EXIF, container metadata, or file mtime as fallback),
dimensions, duration, codec, size, content hash, perceptual hash, manual tags, notes,
and lineage if they came from a compression.

Captured date can be **overridden manually** when it is wrong or missing; the preview panel
shows where the value came from so a guess is never mistaken for metadata.

### Favorites

Favorite is a first-class property, not a tag. It gets `F`, a badge on the thumbnail, a
permanent sidebar entry, and `is:favorite` in queries. It is deliberately binary — no
star ratings, no colour labels — because the value here is marking things instantly
during triage, and any scale needs a rubric you have to remember.

Folders can be favorited too; that is what pins them to the top of the sidebar.

---

## 2. Window layout

```
┌───────────────────────────────────────────────────────────────────────┐
│  ⌕ search / query bar                         [size] [sort] [⧉] [⌘K]  │
├────────────┬────────────────────────────────────────┬─┬──────────────┤
│  SIDEBAR   │           FOLDER HEADER                │ │  INSPECTOR   │
│            │  ┌──────────────────────────────────┐  │2│              │
│  Library   │  │ ▣  Ana            Person   ● WIP │  │0│ selection or │
│  ★ Ana     │  │    instagram  @ana               │  │2│ folder info  │
│   People   │  │    tiktok     @ana_x             │  │5│              │
│    Ana   ● │  │    youtube    —                  │  │ │ tags         │
│    Sara  ○ │  │    2,481 items · 14 subfolders   │  │2│ metadata     │
│   Places   │  │    last added: 5 months ago      │  │0│ notes        │
│            │  └──────────────────────────────────┘  │2│ actions      │
│  Tags      │                                        │4│              │
│  ★ Favorites│           MEDIA GRID                  │ │              │
│  Searches  │      (justified, virtualized)          │2│              │
│  ─────     │                                        │0│              │
│  Sorting   │                                        │2│              │
│    Box  142│                                        │3│              │
│  Pending  8│                                        │ │              │
│  Trash     │                                        │↕│              │
└────────────┴────────────────────────────────────────┴─┴──────────────┘
                                                        ↑
                                              timeline scrubber
```

**Sidebar** — collapsible sections. Library (folder tree), Tags, Favorites, Saved
Searches, then a divider and the three queues: Sorting Box, Pending Review, Trash, each
with a count badge. Pinned folders float above the tree. Each folder shows a coloured
status dot. Folders accept drops. Right-click for new folder, rename, edit tags, set
cover, set status.

**Timeline scrubber** — a thin strip down the right edge of the grid, marked with years
and months. Dragging it jumps to that point in the sort order. At 40k+ items this is the
difference between a browsable library and an endless scroll.

**There is exactly one scrollbar.** The scrubber *is* the scroll affordance — the native
scrollbar is hidden with `scrollbar-width: none` while the scroll container stays fully
functional (wheel, keyboard, and programmatic scrolling all work unchanged). Showing both
is redundant and looks unfinished.

**Panels are resizable.** The sidebar and the preview panel both have drag handles, with
a sensible minimum width and a maximum of roughly half the window. Widths persist between
sessions alongside window geometry. Double-clicking a handle resets that panel to its
default width.

**The native context menu is suppressed everywhere.** Right-click opens the app's own menu
appropriate to what was clicked — a folder, an item, a selection, or empty space. A
WebView's default menu appearing in a desktop app is a bug, not a placeholder.

**Folder header** — appears when viewing a folder. Cover thumbnail, title, archetype
badge, labelled fields edited inline (click the dash, type, done), flags as chips, counts.
This is where you fill in Instagram handles. Collapsible; collapsed state is remembered.

**Grid** — justified rows, sized by a slider. Video items show a duration badge and
scrub through their sprite strip on hover. Selection is click, shift-click for range,
ctrl-click to toggle, drag for marquee. Sort by captured date, added date, size,
duration, or random.

**Preview panel** — the right panel, toggled with `I`. Single-clicking an item shows it
here at a usable size, roughly a third to a half of the window and drag-resizable. This is
the primary way media gets looked at: the workflow is comparing and triaging against the
grid, not presenting one image at a time, so the grid must stay visible while you inspect.

The panel is preview on top, details below:

- **Preview** — the image or video, fit to the panel. Video plays inline, muted, **looping
  by default**. Click the preview to go fullscreen.
- **Details** — filename, dimensions, duration, codec, size, dates, source URL if it came
  from a download.
- **Tags** — inherited render greyed, manual render solid, so it is always obvious which
  came from where. Multi-selection shows shared tags and allows bulk add/remove. Tag entry
  is a combobox with autocomplete over existing keys and values.
- With nothing selected, the panel shows the current folder instead.

### Theatre view

A large view rendered **inside the app window** — not OS fullscreen, no display mode
change, no window chrome disappearing. It is a view that takes over the window, with a
back button, and `Esc` also returns.

Opened by double-clicking an item, pressing `Enter`, or the **Fullscreen** button in the
preview panel. Returning puts the grid back exactly where it was, scrolled to the item you
were looking at.

- Left and right arrows, or on-screen chevrons, move through the current filter — the same
  set the grid is showing, in the same order.
- A filmstrip along the bottom shows position and allows jumping.
- *Images* — scroll to zoom, drag to pan, `1` for 1:1 pixels, `0` to fit.
- *Video* — play/pause, scrub, frame-step with arrows, speed control, **loop on by
  default**, and volume that persists between items.
- `F` favorites, `T` tags, `Del` trashes, `A` adds to multi-view.

### Multi-view

Theatre view holds **one item by default and up to twelve**. An **Add** control appends
the current item to the set; each pane has its own remove control. The layout adapts to
the count:

```
 1 → full         2 → side by side      3–4 → 2×2
 5–6 → 3×2        7–9 → 3×3            10–12 → 4×3
```

Every video in the set plays simultaneously, looping, muted. Clicking a pane solos its
audio — one unmuted at a time, because twelve soundtracks at once is noise, not a feature.
Clicking a pane's expand control drops back to that item alone.

**This carries a real performance risk and must be measured before it is built.** Twelve
concurrent video elements can exhaust the GPU's hardware decode sessions and silently fall
back to software decode; twelve 1080p software-decoded streams will saturate the CPU. The
milestone starts with a throwaway check of how many concurrent streams actually hold frame
rate on the target machine.

If the honest number is lower than twelve, the cap becomes that number rather than the
design being forced. Panes beyond whatever decodes cleanly show a poster frame with a
click-to-play control instead of failing silently.

### Animated media

GIFs are indexed as `kind = video` when animated and `image` when static. They stay GIFs
on disk — browsers animate them natively in an `<img>`, so no conversion is needed to view
them, and converting at import would be a silent destructive rewrite of an original.

Converting GIF to MP4 is a **compression preset** (M6), which means it goes through the
same Pending Review queue as everything else: you see the size saving, compare the result,
and choose. Never automatic, never silent.

WebP and APNG follow the same rule.

---

## 3. Search

One bar, focused with `/` or `Ctrl+F`. It accepts plain text and query syntax
interchangeably — see [DATA-MODEL.md](DATA-MODEL.md#query-language).

**While typing**, a dropdown shows instant matches grouped as Folders, Tags, and Labels,
with an item count against each. Enter on a suggestion filters by it. Enter on raw text
runs a full search.

**Results page** is sectioned:

```
Folders (3)
  ┌────────┐ ┌────────┐ ┌────────┐
  │  Ana   │ │ Ana B. │ │ Ana T. │       ← cards: cover, title,
  │ @ana   │ │@ana_b  │ │@anatrp │         matched field, item count
  └────────┘ └────────┘ └────────┘

Media (2,481)                                    [sort ▾]
  ┌──┐┌──┐┌───┐┌──┐┌────┐┌──┐┌──┐
  └──┘└──┘└───┘└──┘└────┘└──┘└──┘        ← justified grid, infinite scroll
```

Folder cards come first and always render in full — there will never be many. The media
section loads a page at a time as you scroll, with skeleton placeholders holding layout
so the page never jumps.

Searching `@ana` matches the folder by its `instagram` label *and* every item beneath it
by inheritance. That is the whole point of the tag discipline and it should feel
instant — the effective-tag cache exists to make it so.

Sidebar clicks compose into the same bar rather than replacing it. Clicking a folder adds
`path:People/Ana`; ctrl-clicking a tag adds `tag:beach`; alt-clicking negates. The bar is
directly editable, so click-driven and keyboard-driven use are the same mechanism.

**Saved searches** — name any query, it appears in the sidebar.

---

## 4. Sorting Box

A real folder at `<root>/Sorting Box/`, watched live. Items appear from:

- The app's **Add Files** picker (`Ctrl+O`)
- **Dragging from Explorer** onto the window
- **Downloads** (M5)
- **Pasting files into the folder** in Explorer — the watcher picks them up

Subfolders inside it are allowed, so you can partially sort without committing.

Files entering the Sorting Box are renamed to UUIDs, hashed, thumbnailed, and checked
against existing content hashes. Exact duplicates of something already in the library are
flagged on arrival rather than after you have already sorted them.

### Triage — fullscreen (default)

```
┌──────────────────────────────────────────────────────────────┐
│                                                    142 left  │
│                                                              │
│                    [ media fills screen ]                    │
│                                                              │
│                                                              │
├──────────────────────────────────────────────────────────────┤
│  a Ana    s Sara    d Dogs    f Food    g Gym    ⌕ more…     │
│  tags: ______________________            ␣ skip   x trash    │
└──────────────────────────────────────────────────────────────┘
```

- **Destination hotkeys** — user-assigned, shown along the bottom. Pressing one moves
  the item and advances. The bar shows pinned destinations first, then most-recently-used.
- `/` opens a fuzzy destination search when the target isn't on the bar. Typing a name
  that doesn't exist offers **Create folder** inline, with an archetype picker.
- `Space` skips, `X` trashes, `T` focuses tag entry, `I` toggles the preview panel.
- `Ctrl+Z` undoes — including across restarts, and undoing a batch as one action.
- `Tab` switches to grid mode.
- Video plays automatically, muted, looping. `M` unmutes.

### Triage — grid mode

The standard grid, scoped to the Sorting Box, with multi-select. Assign a selection by
pressing a destination hotkey or dragging onto a sidebar folder. Use this when forty
photos obviously belong together; `Tab` back to fullscreen for anything needing a look.

---

## 5. Compression

The point is reclaiming storage, so the interface leads with numbers.

**Starting a job** — select items, `Compress`, pick a preset. Presets are named and
editable, wrapping HandBrakeCLI arguments for video and encoder settings for images.
Ship sensible defaults; the settings screen exposes the raw arguments for tuning.

Compressed output is written to `.gallery/pending/`. The original is untouched until you
decide. Every result goes to **Pending Review** — nothing is ever replaced automatically.

### Pending Review

Because review is always manual, the queue must stay tractable. It has two views.

**List view** — the default, and the one that does the work:

```
Pending Review (48)                      reclaimable: 61.2 GB
                                         [sort: savings ▾]
  ▣  clip_2024_06.mp4    4.2 GB → 380 MB   −91%   ✓ keep new  ✗ keep original
  ▣  beach_walk.mp4      1.8 GB → 240 MB   −87%   ✓ keep new  ✗ keep original
  ▣  portrait.png        88 MB  → 71 MB    −19%   ✓ keep new  ✗ keep original
  …
```

Sortable by savings, size, or duration. Multi-select with a bulk **Keep compressed** —
so the eighty obvious 90% wins clear in one action, and your attention goes to the
marginal ones. Clicking a row opens compare view.

**Compare view** — side by side, keyboard driven:

- *Images* — two panes with synchronised pan and zoom, a 1:1 pixel toggle, and a
  wipe-slider mode for A/B on a single image.
- *Video* — two players sharing one timeline. Play, pause, scrub and frame-step move
  both in lockstep. Arrow keys step frames, which is where compression artifacts
  actually show.
- A persistent stats bar: size, bitrate, resolution, codec, and the delta.
- `Enter` keeps compressed, `O` keeps original, `→` next, `Ctrl+Z` undoes.

**Accepting compressed** — the compressed file takes over the item's identity. It gets a
new UUID filename, `derived_from` points at the original, all tags and folder placement
carry across untouched, and the original moves to trash. Nothing about how you find the
item changes.

**Trash** shows total reclaimable space and a purge button. Space is not actually freed
until you purge.

---

## 6. Downloads

A URL field in the app (`Ctrl+D`) and a queue view.

Paste a URL → the app matches it against patterns to pick `yt-dlp` or `gallery-dl` →
downloads into the Sorting Box, or a folder you nominate. Results are auto-labelled with
`source: instagram`, `uploader: @ana`, and the origin URL, all searchable.

Both tools maintain archive files; the app reads and writes them so nothing downloads
twice. The download record also keys on URL, so re-pasting a known link says so instead
of refetching.

**Cookies.** gallery-dl needs browser cookies for most authenticated sources — Instagram
will not work without them. Settings exposes a cookie file path and a browser-import
option, and download failures caused by expired cookies say so plainly rather than
failing with a generic error.

**Downloads are manual by design.** There are no subscriptions and nothing runs on a
schedule. Download history is recorded and searchable, so finding where an item came from
and re-running that URL is easy — but the app never reaches out on its own.

The queue view shows progress, speed, and errors, with retry. **Update tools** fetches
current pinned versions of yt-dlp and gallery-dl — this is a routine action, not a rare
one.

---

## 7. Duplicates

Perceptual hashing for images; sampled-frame hashing for video. Groups are surfaced in a
review screen reusing the compression compare view, with resolution, bitrate, size and
date shown against each candidate.

Choosing a keeper merges the loser's manual tags into it before trashing, so you never
lose tagging work by deduplicating.

---

## 8. Cross-cutting

**Undo** — a persistent journal. `Ctrl+Z` works across restarts and treats a batch
operation as one step. This is what makes fast triage comfortable.

**Command palette** — `Ctrl+K`. Folders, saved searches, settings, and every action.

**Trash** — soft delete. Files move to `.gallery/trash/` preserving their relative path;
the database row keeps a `deleted_at`. Restorable until purged.

**Keyboard** — every primary action has a binding, and destination hotkeys are
user-assigned. A `?` overlay lists them.

**Single instance** — a lock file in `.gallery/` prevents two copies opening the same
library.

**Blur toggle** — one key blurs every thumbnail and preview instantly, and un-blurs on
the same key. No PIN, no hidden state, nothing persisted. It is a panic button for
someone walking past, and is not represented as security anywhere in the interface.

---

## 9. Utility screens

**Storage dashboard.** Reclaiming disk space is the reason compression exists, so it
gets a screen that leads with the numbers: total library size, largest folders, largest
individual files, how much of the library has never been compressed, and an estimate of
what compressing it would save. Every list is actionable — select the worst offenders
and queue them straight into compression.

**Tag management.** Rename a tag and it propagates everywhere. Merge two tags into one.
Delete unused ones. See usage counts. **Aliases** map several values to one canonical tag
(`ana_official` → `@ana`) so a search finds it either way. Without this screen the tag
vocabulary rots within a year, and the whole searchability premise goes with it.

**Export.** Because filenames on disk are UUIDs, getting media *out* needs a first-class
path. Select any items, choose Export, pick a destination, and they are copied out with
reconstructed names — folder title, captured date and an index — using a configurable
pattern. Originals are never touched.

The same name reconstruction then upgrades **copy-to-clipboard** (§1 *Item operations*),
which until now puts files on the clipboard under their UUID names: stage a copy under the
reconstructed name, put that on the clipboard, and pasting anywhere produces something
readable. One implementation, two surfaces — which is why the naming was left out of M2.1.

**Integrity check.** Re-hashes every file and reports anything missing, moved unexpectedly,
or corrupted. Run it after moving the library to a new machine to confirm the copy was
clean, or on a schedule if you are feeling careful.

---

## 10. First import

<a id="first-import"></a>

Pointing the app at a library it has never seen renames every file to a UUID. It happens
once, before the library is ever shown.

**It is part of the startup flow, not a dialog over the app.** The sequence is full-window
screens, in the same visual language as the folder picker — no modal floating above a grid
that is already loading thumbnails of files about to be renamed. Nothing is indexed, no
thumbnail is generated, and no `.gallery/` content is written until the rename has run.
The library is normalised first, then opened.

```
  ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌─────────┐
  │  Choose  │ ──▶ │  Review  │ ──▶ │ Progress │ ──▶ │ Gallery │
  │  folder  │ ◀── │          │     │          │     │         │
  └──────────┘ Cancel─────────┘     └──────────┘     └─────────┘
```

**Choose folder** — the existing picker.

**Review** — one screen, and the only one that asks anything:

- What was found: file count, total size, anything unreadable.
- What will happen, in one sentence: files are renamed to UUIDs, original names are kept
  and shown in each file's details.
- A short before/after sample — five rows, not a full manifest.
- One checkbox: *I have a backup of this folder.* Nothing proceeds without it.
- **Cancel** returns to the folder picker. **Import** starts.

**Progress** — rename, then index, then thumbnails, as one continuous readout. Verification
runs here silently: a random sample is re-hashed and counts are confirmed, surfaced only if
it fails.

Then the gallery opens.

### Keep it short

The flow above is deliberately two screens and one checkbox. An import wizard that explains
itself across six panels reads as nervous, and a user who is asked to confirm four times
stops reading by the third. There is exactly one thing worth interrupting for — that a
backup exists — because there is no undo.

### No reversal

There is no reversal script and no undo for the import. The app owns filenames; that is
locked decision 5, and the rename is normalisation rather than an edit the user made.

Original filenames are **metadata**. They live in `item.orig_name`, are searchable, and
appear in each file's details alongside dimensions and dates. `library.jsonl` still records
the uuid-to-original mapping as part of the disaster-recovery export, so the information to
reconstruct original names always exists — but reconstructing them is not a feature, and no
tooling ships for it.

This is why the backup checkbox is the one confirmation that stays.

### Repairing later

Settings keeps a **Normalise filenames** action: it finds anything in the library that is
not UUID-named and renames it, through the same single confirmation. That is for when
something has drifted, not the normal path — see below.

### After the first import

The import is a bulk operation with a confirmation because it touches thousands of files at
once. **Ongoing arrivals are not that**, and must never require it. Once a library is
marked imported, anything entering it gets a UUID name as part of being taken in — no
prompt, no screen, no backup gate. It is one file the user just added, and its original
name is preserved in `orig_name` like any other.

Renames are not undoable, here or at import. Moves, deletes and compressions are — those
are decisions the user made, and they go through the journal. A rename is the app applying
its own naming rule to a file it has taken ownership of.

### The library is live

**There is no "Re-index" button.** Indexing is not an action the user takes; it is
something the app does because the folder changed. A control that asks the user to keep
the database in sync with the disk is the app admitting it cannot.

The library root is watched continuously. Anything that appears, changes or disappears —
whether the app did it, Explorer did it, or another tool did it — is picked up and
reflected in the grid without a refresh:

- **Appears** → renamed to a UUID, hashed, probed, thumbnailed, and it shows up in place.
- **Changes** → re-hashed and re-thumbnailed. The item keeps its identity; content hash is
  updated rather than treated as a new file.
- **Disappears** → retired from the view.

Progress surfaces as a transient readout — *Indexing 42 items…* — that goes away on its
own. It never blocks, and it never needs dismissing.

`.gallery/` is excluded from watching, and paths the app is itself mid-write on are
suppressed so its own work does not feed back in.

**Files still being written are left alone.** A large video copied in from Explorer
generates events long before it is complete; the watcher waits for size and mtime to
settle before touching it. Indexing a half-copied file would record a hash for something
that no longer exists a second later.

**If the watcher fails, the app says so and falls back.** Windows drops change
notifications when too many arrive at once, and the OS reports that overflow rather than
hiding it. On overflow or watcher error, a full reconcile walk runs and the readout says a
rescan is happening. Silent divergence between disk and database is the one outcome that
is not acceptable — a visible rescan is fine.

Files arrive by two routes, and only one of them needs renaming at all:

**The app creates the file.** Downloads (M5), compressed output (M6), converted GIFs (M6),
anything written by a job. These are named `<uuid>.<ext>` at the moment they are written.
There is no rename step because there was never a wrong name.

**Something outside the app creates the file.** Pasted into a folder in Explorer, dropped
onto the window, added through the file picker, or moved in by another tool. The indexer
and the filesystem watcher rename these on the way in, as part of indexing — the same code
path the wizard drives in bulk, applied to one file at a time.

The rule that makes this coherent: **renaming is a property of indexing, not a milestone.**
The import is that same operation run over an entire pre-existing library at once, behind
one confirmation because the scale makes it worth pausing for. Nothing else needs it.

M1's strict read-only stance was a milestone constraint that ends here. From this point
the app owns filenames for everything it takes in.
