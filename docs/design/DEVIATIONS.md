# Deviations and what is not built

**The drawing is the specification for how GGallery looks.** This file is everything that
qualifies it: the handful of places we deliberately differ, the parts not built yet, and
the questions the drawing does not answer.

Read it alongside [`GGallery.dc.html`](GGallery.dc.html). Line numbers throughout are that
file's — markup runs to 1488, and the state, sample data and accent definitions are in the
`data-dc-script` block from 1489.

**Sections 2–5 shrink as work lands.** When a surface is built to the drawing, delete its
row. When this file holds nothing but section 1, the drawing and the build agree.

---

## 1. Deliberate deviations — permanent

Four, and only four. Anything else that differs is a bug or an unbuilt item, not a choice.

- **Masonry columns is not built.** The layout control ships with justified rows and uniform
  grid. Masonry is column-major, so it has no rows, and the grid's windowing and tile
  recycling are built on rows — it would mean a second layout model, a second windowing path
  and a second recycler through the one piece measured against a 100k library. The drawing's
  own masonry uses CSS `columns`, which lays out every item at once and is what
  virtualisation exists to avoid.
- **Sort offers more than the drawing draws.** The drawing's control lists four orders; the
  specification names captured date, added date, size, duration and random. The drawing is
  the authority on how a control looks and where it sits, not on how short a list may be.
- **The triage screen's standing `SEND TO` bar is held for M4.** Not an appearance question:
  the drawing makes the Sorting Box a screen with its own header where the build makes it a
  scope of the ordinary grid. That is structural.
- **The mockup's own scaffolding is not design.** The `MOCKUP CONTROLS` bar (40–58), the
  clickable step indicator on the import screen, and the `unpkg.com` fetches for React and
  icons are how the drawing renders itself. `lucide-react` is already the icon source.

---

## 2. Built, but not yet matching the drawing

### 2.1 Window bar — `src/components/WindowBar.tsx`

Drawing 62–96.

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Height | 36px | `h-8` (32px) | layout |
| Mark | 18px rounded square, `--rsd` fill, `--ln` border, "G" | `<Mark className="size-4" />` | appearance |
| Divider after the mark group | 1px `--lns`, inset 8px vertically | none | appearance |
| Caption buttons | 46px wide, close hovers `--dngr`/white | `w-11` (44px), same hover | appearance |
| Maximise glyph | always `square` | `Square`, swapping to `Copy` when maximised | drawing silent — §5 |

The centred search field and the `people › ana` breadcrumb are **ahead** — see §3.

**The window bar holds nothing else.** No tile size, no scope toggle, no *Open pane*
button, no library name or path. That is decision 28 and DESIGN §2 held exactly.

### 2.2 Navigation panel — `src/features/nav/Nav.tsx`

Drawing 100–232 (expanded 121–231, folded rail 102–119).

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Width | 232 open / 44 folded | `NAV_DEFAULT` 200 / `NAV_FOLDED` 44 | layout |
| Header row (44px) | `LIBRARY` mono heading at the left, fold button at the right | three inert placeholder `IconButton`s, spacer, fold button | layout |
| Group headings | mono `600 11px`, `.14em`, `--fgd`, `20px 12px 6px` | `GroupLabel`: mono 12px, normal weight, `.1em`, `pt-4 pb-1 px-3` | appearance (but 11px is taken) |
| `FOLDERS` heading | carries a right-aligned folder count | no count | **new** — §2 |
| *New Root Folder* button | full-width, under the `FOLDERS` heading | none; creation is the tree's context menu or the pane's Folders mode | **new** — §2 |
| Root rows | 32px, radius 4, active `--act` bg + `--ac` text/icon/badge | identical shape via `ROW_ACTIVE` | already aligned |
| Everything row | carries a count | `countFor` returns `undefined` for it | **new** — §2 |
| Everything icon | `images` | `LayoutGrid` | appearance |
| Tree row indent | 15px per level | 14px per level | appearance |
| Tree chevron hover | translucent white overlay `rgba(255,255,255,.10)` | `hover:bg-hover` | appearance |
| WIP dot | `--warn`, 6px | `bg-fg-mid`, 6px | appearance (caveat below) |
| Tree row hover actions | `+` new subfolder and `⋯` more, revealed on hover in a 48px cluster | none — both are in `FolderMenu` only | layout + **new** |
| Pinned rows | no chevron (26px spacer), no WIP dot, no hover actions | reuse the full `FolderRow` | layout |
| Footer, indexing | mono line **plus** a 3px progress bar | mono line only | layout |
| Footer, idle | `41,236 items · 1.84 TB` | renders nothing when idle | **ruled: build the drawing** |
| Footer, failures | `triangle-alert` glyph + count, danger-tinted | count alone, danger `Button` | appearance |
| Resize handle | 7px hit area over a 3px line, `--ac` on hover | `.resizer` — identical | already aligned |

**Caveat on the WIP dot.** Statuses are user-defined and user-**recolourable** (DESIGN §1
*Folder status*). The drawing hard-codes the mark to `--warn`. That is defensible for the
tree, where the point is one fixed mark for one fixed key, but it is not a general rule for
status colour — see the band's status chip below.

The `QUEUES` group (Pending Review, Downloads, Duplicates, Trash) and `SAVED SEARCHES` are
**ahead** — §4.

### 2.3 Folder band — `src/features/folder/FolderBand.tsx`

Drawing 241–336.

**Collapsed row.** 44px, rotating chevron, 16px/600 title, counts in mono prose. All four
match the build. Two differences:

- The drawing Title-Cases its counts — `2,481 Here · 612 Below · 2 Subfolders · Added 4d
  Ago`. The build lowercases them. This is one instance of a systematic difference — see
  §2.10.
- **Status chip** — the drawing draws a 26px pill hard-tinted `--warn` with a `chevron-down`
  affordance (249). The build draws a 28px pill in the status's own user-chosen colour with
  no chevron. The chevron is a real improvement (the chip is a dropdown and does not say
  so); the hard-coded colour is not, for the reason above. **appearance**, split.

**Right side.** Scope segmented control, sort button, layout segmented control, tile-size
slider, pin, overflow menu → **ruled: build the drawing**.

**Expanded panel.**

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Cover | 88×88, radius 6, `--sunk` ground | `size-14` (56×56), radius 5, `bg-raised` | layout |
| Clear-cover button | always present | only when a cover was chosen | drawing silent — §5 |
| Breadcrumb | mono 12px, `chevron-right` separators, ends with the folder itself | `Breadcrumb` — mono 12px, `/` separators, ends with the folder itself | appearance |
| Labels and tags | **two rows, each under a mono heading** | one chip row | **ruled: build the drawing** |
| Label chip | rectangle; key half `--sunk`, value half `--rsd`; inherited = both halves `--sunk`, `--lns` border | `FieldChip` — key half `bg-ground`, value `bg-raised`; muted = both `bg-ground`, `border-line-soft` | appearance — **exactly right once `--sunk` exists** |
| Tag chip | pill, 26px, `--rsd`/`--ln`; inherited `--sunk`/`--lns`, no remove button | `Chip` — pill, 28px, same colours modulo `--sunk`; muted has no remove button | appearance (26px — taken) |
| Add controls | *Add Label* rectangular dashed, *Add Tag* pill dashed | both rectangular dashed (`AddChipButton`) | appearance |
| Notes | 30px one-line button, hover gets `--ln` border + `--rsd` | `h-8`, same hover | already aligned |

### 2.4 Grid — `src/features/grid/Grid.tsx`, `Tile.tsx`, `styles/index.css`

Drawing 393–479 (justified 420–442), tiles 423–439, scrubber 477–479, selection bar 482–501.

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Padding / gap | 8 / 8 | `PADDING` 8 / `GAP` 4 | layout |
| Tile radius | 5px | 4px | appearance |
| Tile hover | `border-color: --fgd` | identical | already aligned |
| Selected tile | `--ac` border **+ a full `--act` wash + a 20×20 accent check badge top-left** + `0 0 0 1px rgba(0,0,0,.4)` | accent border + `color-mix(accent 18%, raised)` | appearance for the wash; the check badge is **ruled: build the drawing** |
| Favourite badge | 22×22, `rgba(8,9,11,.72)`, `star` glyph in `--ac` | `.tile-fav`, the literal character `★` | appearance |
| Duration badge | 19px, mono, with a leading `play` glyph | mono, no glyph | appearance |
| `GIF` corner label | bottom-left, 10px mono, `rgba(8,9,11,.66)` | none | **new**, but needs `ext` on `GridItem` — §2 |
| Scrubber | 16px channel, **`--sunk`** ground, `--lns` left border, thumb `--ln` → `--fgd` on hover, `cursor: pointer` | `.scrubber` — same but `--color-ground`, and no pointer cursor | appearance; the pointer ships — decision 25 |
| Empty state | glyph + heading + one sentence + *Add Files* / *Download From URL* | one line of grey text | layout; the two buttons are §3 |
| Indexing banner | sticky at the top of the grid: spinner, sentence, `18,402 / 41,236 · 214/s · ~2m left`, a bar, *Pause* | nothing here; the nav footer carries indexing | layout + **new**, partly unbacked — §2 |
| Drag ghost | floating chip: thumbnail, `Moving 12 Items`, `→ trips / lisbon` | the browser's native drag image | **new** — §2 |

**Selection bar** (482–501). The drawing runs *Select All · Invert · Clear ‖ Move To… ·
Add Tag… · Favourite · Compress… · Export… ‖ Delete* then `12 Selected · 480 MB`, with the
scrubber's channel continued to the window edge. The build has *Select all · Clear ‖ Move
to… · Delete* and a count, and already continues the channel. **layout**, plus: Invert, Add
Tag… and Favourite are §4; Compress… and Export… are §4; the `· 480 MB` needs `size_bytes`
on `GridItem`, which the payload does not carry.

**Subfolders never appear in the main grid.** Held — see §1.

### 2.5 The pane — `src/features/pane/*`

Drawing 974–1236.

**Header** (988–1041). The drawing's order is fill-window arrow · mode-specific content ·
**mode switcher as one bordered segmented control** (30×24 buttons) · divider · fold. The
build has the same order with the switcher as three separate 32×32 `IconButton`s. **layout**
for the grouping; the 30×24 button size is taken.

**Preview mode.**

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Header content | rotating chevron + **filename** | rotating chevron + **dimensions · size** | layout |
| Details block | below the header, `max-height: 420px` | below the header, `max-h-[45%]` | layout |
| Detail rows | 8 rows, `110px 1fr`: File Name, Original Name, Dimensions, Size, Codec, Captured, Added, Source | File Name, Original Name, Duration, Codec, Created, Added, Source | layout + wording (`Captured` vs `Created`) |
| Labels and tags | **two blocks, each under a mono heading**, the labels one asserting `INHERITED FROM FOLDER` | one chip row, inherited-vs-manual decided per tag by `originId` | **ruled: build the drawing** (and the heading over-claims — an item can carry its own label) |
| Media stage | `--sunk` | `bg-ground` | appearance |
| Zoom readout | bottom-right, 26px rectangle, mono, `rgba(15,17,20,.86)` (= `--sunk`/86%) | bottom-right rounded-full pill, `bg-ground/90`, mono, with a glyph | appearance |
| **Item action bar** | 44px under the media: Favourite · Move To… ‖ Reveal · Copy · Open With · Blur · Delete · More | none — these live only in the right-click `ItemMenu` | layout + **new** — §2 |
| Filmstrip | 76px, 86px thumbs, 5px gap, chevrons overlaid in gradient fades at either end, **`class="noscroll"`** | resizable, chevrons, and a scrollbar | layout; the missing scrollbar is **ruled: build the drawing** |

**Grid mode** (1007–1012, 1148–1159). Drawing header: `trips / lisbon` + `616 items`.
Drawing footer: `Drop Items Here To Move Them Into This Folder`. Built header: a
folder-picker button carrying a breadcrumb; built footer: a tile-size slider and a count.
**layout** — and note the drawing **drops the tile-size control**, which DESIGN §2 *Grid
mode* requires ("with its own sort and tile size"). It must not be dropped in M2.8c.

**Folders mode** (998–1005, 1124–1146). Drawing: Up button + breadcrumb header, 3-column
tiles with square covers, title and count, a dashed *New Folder In Trips* tile, and a 44px
filter box pinned at the bottom. The build has all of this, with `auto-fill minmax(120px)`
instead of a fixed 3 columns, and already previews `610 → 616` during a drag
(`FoldersMode.tsx:340`). **appearance** only.

**Closed strip** (976–985). 44px header holding the reopen button, then the mode icons
below. Identical to `PaneStrip`. **Already aligned.**

### 2.6 Details panel

Covered under Preview mode above — the drawing has no separate details surface.

### 2.7 Settings — `src/features/settings/SettingsPanel.tsx`

Drawing 1415–1480.

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Shape | 1120×760 modal, 240px section list on `--bg`, 52px content header carrying the section title + close | `Dialog width={720}`, 170px section nav, no per-section header | layout |
| Section rows | 32px, icon + label, active `--act`/`--ac` | 32px, label only, active `bg-accent/15 text-accent` | layout (icons) |
| Accent picker | four 132px cards, each showing hue · deep · a `--rsd` chip bordered in deep, with a paragraph above explaining what the accent carries | 3-column grid of 32px buttons with one dot each | layout |
| Preference rows | label + hint + a switch | none | layout + §3 |
| Library block | path · item count · total size, with *Change Library…* | `Action` row with the path | layout |

The section list itself is **ahead** past the four built sections — §4. **There is no
`Switch` primitive in `src/components/ui/`**; the drawing needs one, and nothing in the app
would consume it until those preferences exist.

### 2.8 Menus and toasts — `src/components/ui/menu.tsx`, `src/components/Toaster.tsx`

Drawing 953–962 (menu), 947–952 and 1406–1413 (toast).

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Menu surface | 230px, 5px padding, radius 6, **`--rsd`** | `min-w-[212px]`, 4px padding, radius 6, **`bg-panel`** | appearance |
| Menu rows | 30px, radius 4, hover `--hov`, Delete in `--dngr` | 32px, radius 4, `data-[highlighted]:bg-hover`, Delete in danger | appearance (30px — taken) |
| Shortcut column | mono 11px `#565d68` | mono 12px `text-fg-dim` | appearance (11px — taken) |
| Toast position | centred, 28px off the bottom | bottom-right viewport | layout |
| Toast surface | `--rsd`, leading `check` glyph in `--good` | `bg-panel`, no glyph | appearance |
| Toast body | sentence with the destination in mono | sentence, plain | appearance |

### 2.9 Banners

**ffmpeg** (338–346). The drawing adds an `info` glyph in `--warn`, mono `ffmpeg` and
`tools/`, and two buttons — *Download Tools* and *Dismiss*. The build is one line of text on
`bg-raised`. **layout**; *Download Tools* is M5 (§3); *Dismiss* would need a dismissed flag
that does not exist.

**Indexing failures** (348–391, plus the pane's failure mode 1014–1019 and 1161–1204).
`FailureList.tsx` renders a count, a grouped-by-message summary and a plain table of
name / stage / size / error. The drawing renders a header with a **retryable · damaged ·
skipped** breakdown, *Retry All Retryable* and *Export List*, then one card per failure
carrying a `kind` chip, a reason, a **hint**, and five per-row actions (Retry, Reveal, Open
With, Ignore, Delete) — and a whole **pane mode** for the selected failure with a "no
preview" stage, the reason, a *WHAT TO DO* block, and a metadata grid.

Classed **layout** for the shape, but be plain about the rest: `IndexFailure` carries
`jobId, stage, name, error, attempts, sizeBytes` and retry is all-or-nothing
(`library.retry`). Per-failure retry, an ignore list, a failure taxonomy, a remediation
hint, and *Export List* are all **new backend**. This is not an M2.8c restyle; it is a
feature with a drawing attached, and it needs its own decision about where it belongs.

### 2.10 First import — `src/features/import/ReviewScreen.tsx`, `ProgressScreen.tsx`

Drawing 1239–1335. **`docs/design/SOURCE.md` lists this among the drawn-ahead screens; it is
not — it is built** (M2.6a).

| | Drawing | Built | Class |
| --- | --- | --- | --- |
| Step indicator | three numbered steps across the top | none | layout |
| Review copy | 26px heading + one paragraph | 22px heading + path + one paragraph | appearance |
| Findings | four stat cards: Files Found, Total Size, Directories, Unreadable | a mono table by kind, plus an unreadable line | layout |
| Backup gate | danger-bordered block: *There Is No Undo For This*, explanation, checkbox | a bordered label with the checkbox | layout |
| Actions | *Rename And Import* · **Dry Run First** ‖ *Cancel* | *Cancel* ‖ *Import* | **ruled: build the drawing** |
| Progress | four phase rows, each with an icon, a bar and a meta column | a single readout | layout |
| Choose-folder step | heading, *Choose Library Folder*, a "reopen" row | `Welcome` in `App.tsx` — same three things | appearance |

`StorageMigrationScreen` is not drawn at all — §5.

### 2.11 The Components screen — the drawing's own spec sheet

Drawing 847–967. Not a surface; it is the vocabulary M2.8b is built from, and it states
rules the prose does not:

- **Buttons 28 / 32 / 38, and "every variant has a surface at rest"** — matches decision 25
  and `button.tsx` exactly. Variants: default, accent, good, danger, disabled at `opacity:.4`,
  icon 32, icon 38. The build has all of these.
- **"Text is Title Case."** Systematic, and the build is sentence case throughout. Title Case is taken.
- **"Icon and label are centred together as one group."** The build's `cva` base already does
  `justify-center gap-1.5`.
- **"Counts are mono with tabular numerals."** Already true.
- **"Label is a rectangle, tag is a pill."** Already true (`FieldChip` vs `Chip`).
- **"Inherited chips are recessed and carry no remove button."** Already true.
- **"Sub-controls use translucent overlays."** With the reason: *"The old grey hover on the
  chevron vanished on the accented selected row. A translucent white overlay reads on every
  row state, so one rule covers all of them."* The build uses `hover:bg-hover` on the tree
  chevron, which is the exact defect named. **appearance, and worth taking.**
- Field: 32px, radius 4, `--bg` ground, `--ln` border, **focused border `--ac`**. The build
  focuses to `--color-accent-d` (the deep variant). **appearance.**
- Slider: track `--rsd`, range `--ac`, thumb 14px in **`--fg` with a 2px `--pnl` ring**. The
  build's thumb is `bg-accent` with `border-accent-d`. **appearance.**
- Checkbox: 18px, radius 4, checked = `--ac` fill with a `#0f1114` tick. The build is 16px,
  radius 3, `text-ground` tick. **appearance.**
- Switch: 34×19, `--ac` track and `#0f1114` knob on; `--rsd` track and `--fgd` knob off.
  **No primitive exists.**

---


---

### 2.12 Carried from the token pass

Three things the token pass could not reach, and one measurement to settle.

- **`--color-fg-faint`.** The drawing uses four greys darker than `--fgd` — `#5c636e`,
  `#565d68`, `#4d545e`, `#414852`, 18 uses, all small print. The token pass rounded all four
  *up* into `--color-fg-dim`, which makes them louder than drawn. One token at `#5c636e`, a
  step below `fg-dim`, routed as each surface is reached.
- **The `wip` status colour.** The tree's dot is `--color-warn` (`#c9963f`); migration 002
  seeded the status row `#eab308`, so the band's status chip and the tree's dot disagree for
  the default vocabulary. Needs a **new** migration that updates the row only where its
  colour is still exactly the seeded value — statuses are a vocabulary the user owns, and
  someone who has recoloured `wip` keeps their colour. Editing 002 reaches no library that
  already ran it.
- **Three buttons measure 31px** on `#kitchen-sink` where the scale says 32. Probably a
  border rounding off-by-one. Find which, then fix it or record why 31 is right.

---

## 3. New controls the data already supports

Everything here could be built against what the app already stores and already exposes. That
is not an argument that it *should* be — only that nothing is blocking it.

| # | Control | Backing | Note |
| --- | --- | --- | --- |
| N1 | **Sort control** — captured date, added date, size (DESIGN §Grid adds duration and random) | `item.captured_at`, `added_at`, `size_bytes`, `duration_ms` all exist | `db::items::list` hard-codes `ORDER BY COALESCE(captured_at, mtime) DESC, id DESC` (`items.rs:443`). Needs a sort argument through `list_items` and a `UiPrefs` field. **Placement settled: it goes in the band.** |
| N2 | Folder count beside the `FOLDERS` heading | `library.folders.length` | |
| N3 | A count on the **Everything** row | `library.items.length` — already passed to the band as `itemCount` | |
| N4 | *New Root Folder* button under the `FOLDERS` heading | `ops.createFolder(null, …)` | Decision 22 and 23 both argue for a visible control here |
| N5 | Tree row hover actions: `+` new subfolder, `⋯` more | `ops.createFolder`, `FolderMenu` | Duplicates the context menu deliberately — decision 23 |
| N6 | Selection bar: **Invert**, **Add Tag…**, **Favourite** | `selection.invert`, `ops.addItemTag`, `ops.setFavorite` | All three exist and are reachable only by menu or key today |
| N7 | Pane item action bar: Favourite, Move To…, Reveal, Copy, Open With, Delete | `ops.setFavorite`, `moveItems`, `revealItem`, `copyItemFile`, `openItem`, `deleteItems` | *Blur* is M9 — §4. Strongest decision-23 argument in the drawing |
| N8 | Selected-tile `--act` wash and the accent tick | pure CSS | The tick badge ships — decision 26 |
| N9 | Indexing progress **bar** in the nav footer | `Progress` carries `items`, `pending`, `running`, `completed` | The bar is backed. The **rate (`214/s`)**, the **ETA (`~2m left`)** and **Pause** are not |
| N10 | Drag ghost naming count and destination | `dnd.tsx` already tracks `dragging`; a custom `setDragImage` is available | |
| N11 | `GIF` corner label on a tile | `item.ext` exists in the database | **Not** on `GridItem` — needs a payload field |
| N12 | `· 480 MB` in the selection bar | `item.size_bytes` exists in the database | **Not** on `GridItem` — needs a payload field |

---

## 4. Drawn ahead of its milestone

**M2.8d's input. None of this is built now.** A surface with no backend is a prop.

| Milestone | What the drawing covers |
| --- | --- |
| **M3 — Search** | The window bar's centred query field with its `Ctrl F` hint and clear button (78–89); the window bar's `people › ana` breadcrumb (70–76); the whole Search screen — results header, removable query-term chips, *Add Term*, *Save This Search*, a relevance sort, `FOLDERS` cards then a `MEDIA` grid, skeleton tiles and a "loading the next page" line (504–559); the `SAVED SEARCHES` nav group (201–211). Also decision 32: every chip becomes clickable here. |
| **M4 — Sorting Box and triage** | The Triage screen (561–597) — its own header replacing the folder band, `142 Unfiled · 6.2 GB`, a selected-count pill, *Open The Fast Culler*, and a permanent `SEND TO` destination bar; the Fast Culler overlay (1368–1404) — media stage, hotkey row, tag field, Skip / Trash / Undo Last; the `Trash` queue row; Settings §*Triage Hotkeys*. **The `SEND TO` bar is held — see §1.** |
| **M5 — Downloads** | The Downloads screen (599–649) — URL bar with tool auto-detection, destination picker, queue rows with per-tool and per-status chips, *Update Tools*, *History*, the expired-cookie banner; the `Downloads` queue row; the ffmpeg banner's *Download Tools*; Settings §*Downloads And Cookies* and §*Sidecar Tools*. |
| **M6 — Compression and review** | Pending Review (651–688) — the savings table, *Keep New* / *Keep Original*, *Keep Compressed For All Above 80%*; the pane's **Compare mode** (1207–1234) with its synced badge, stat grid and 1:1 / Wipe controls; the selection bar's *Compress…*; Settings §*Compression Presets*; the `Pending Review` queue row. |
| **M7 — Duplicates** | The Duplicates screen (690–734) — similarity segmented control, group cards with a phash header and per-candidate metadata, *Keep The Best Of Every Group*; the same Compare pane mode; the `Duplicates` queue row; the culler's *7 Exact Duplicates On Arrival*. |
| **M8 — Utility screens** | Storage (736–795) — four stat cards, *Largest Folders*, *Largest Files*, *Run Integrity Check*, *Purge Trash*; Tags (797–845) — the tag table with kind, use count and aliases, merge / rename / alias, *Delete Unused*; the selection bar's *Export…*. |
| **M9 — Polish** | The *Blur Everything* control in the pane's action bar and in Multi-View; Settings' preference rows — Blur, Loop Video, Scrub Tiles On Hover, Reduce Motion, Confirm Before Deleting. |
| **M10 — Multi-view** | The Multi-View screen (1337–1366) — slot-count segmented control, adaptive grid, per-pane progress, the `AUDIO` solo badge, *Shared Timeline*. Note PLAN §M10: **the cap is measured before it is built**, and the drawing's `12` is not that measurement. |

Not a milestone: the drawing's **MOCKUP CONTROLS** bar (40–58) and the import step
indicator's clickable steps are canvas scaffolding, not design.

---

---

## 5. What the drawing does not say

The drawing has twelve screens and seven states, and the seven states are `normal`,
`indexing`, `empty`, `failures`, `drag`, `selection`, `noffmpeg` (1514–1522). Everything
below has no picture, so **DESIGN §Prior art still governs** — a citation has to be lookable
rather than recalled, and the drawing is now the first place to look only where it says
something.

- **Long-title truncation.** Every title in the drawing is short. `text-overflow: ellipsis`
  is set on nav rows, tree rows, the band title and the pane header, so single-line
  truncation is implied — but nothing shows a 400-character title (DESIGN §1 *Folder names*
  permits one), a title that truncates in a breadcrumb, or whether a truncated title gets a
  tooltip. No tooltip appears anywhere in the drawing.
- **Error states.** No failed dialog, no failed operation, no toast in `danger` tone (the
  drawing's only toast is a `--good` success), no "could not undo" state — all of which
  `Toaster.tsx` renders today. `library.error` and `lowercaseMergeReport` have no picture.
- **In-progress states.** No button in a pending state, no disabled-because-running control,
  no skeleton outside the Search screen. The build's `busy` on the Review screen, and
  `library.loading` on the Welcome screen, are undrawn.
- **Empty states other than the grid's.** No empty tree (the build renders "No folders yet.
  Right-click here to make one."), no empty Folders mode, no empty filter result in Folders
  mode, no library with no accents… The grid's is the only one drawn.
- **Focus.** Not one `:focus-visible` ring anywhere. The single global rule in `index.css`
  stands unchallenged.
- **Dialogs other than Settings.** No confirm-delete, no move-items picker, no new-folder
  dialog, no folder picker, no archetype editor, no status editor, no tag editor — all built,
  none drawn. `Dialog.tsx` and `Dialogs.tsx` have no citation from the drawing.
- **The storage-migration screen.** Not drawn at all.
- **Motion.** The drawing carries two transitions — `transform 120ms ease-out` on the two
  rotating chevrons (170, 244, 993). Nothing else animates, and decision 27's 180ms layout
  tier has no picture. The panel folds, the band reveal, the details reveal and the maximise
  tween are all undrawn.
- **The maximised and folded combinations.** `navFolded` and `paneOpen` are drawn
  independently; `maximised` changes only the pane header's arrow (1987–1991), never the
  layout.
- **Multi-select on anything but tiles.** The tag table's checkboxes are drawn; row-range
  selection in the tree is not.
- **The window's own minimum size, and what the band does when the window is narrow.** The
  drawing is 2560×1440 and only 2560×1440. The band's six control groups at 1280px is
  undrawn.

---

