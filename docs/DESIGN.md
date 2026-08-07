# Design

Product and UX specification. See [DATA-MODEL.md](DATA-MODEL.md) for schema and query
syntax.

**Read this first.** GGallery is a gallery viewer and collection organiser, in that order.
The main activity is looking at things: browsing the grid, opening an item, moving through
a folder, comparing two shots. Folders and tags exist so you can find something again;
search so you can find it faster; downloads so there is more to look at; compression and
duplicate detection so the collection stays worth keeping; triage so filing never becomes
the reason you stop adding to it.

Every section below serves the viewing experience. When a decision trades off between
making the app better to look through and making it better to administer, looking through
wins.

## Prior art

**The viewer is designed here. Everything else is copied from something that already
works.** The grid, the pane, the folder band and triage have no adequate prior art, so they
get specified in detail. Settings, the command palette, the tag manager and every other
ordinary surface do have prior art, and inventing a shape for them is how Settings ended up
as four dialogs that each replaced the last.

**A citation has to be lookable, not recallable.** Naming an application is not a
specification: model recall of a specific interface is unversioned, unverifiable, and fails
by confabulating a plausible layout rather than by admitting the gap. So, in order:

1. **A screenshot in `docs/reference/`.** Unambiguous, dated, and readable by whoever
   builds it. This is what makes citing prior art stronger than describing it.
2. **A `shadcn/ui` block.** Real code, fetched through the registry MCP server configured
   in `.mcp.json` and read before use — not remembered.
3. **An application name plus one sentence describing the layout.** The name cross-checks
   the sentence; it never replaces it. If the sentence alone would not be enough, the name
   does not rescue it.

Capturing the screenshot is the user's job and belongs in the prompt that precedes the
milestone, not in the milestone itself.

---

## 1. Core concepts

### Folders

A folder is a record in the database. It has no directory on disk and no location — the
hierarchy is data (PLAN decision 30). It has:

- **Title** — free text, anything. A person, a place, an event, a topic. Lowercase.
- **Archetype** — optional template that pre-creates a set of empty labels.
- **Labels** — key/value pairs, nullable. `instagram: @ana`, `city: lisbon`.
- **Flags** — plain tags. `archived`, `favourite`.
- **Cover** — an item inside it, chosen manually or picked automatically.
- **Count** — items directly inside, and items recursively beneath.
- **Status** — `Active` / `WIP` / `Done` / `Archived`, user-editable set. Renders as a text
  chip in the folder band and is queryable as `status:wip`.

  **One mark, not four.** The tree shows a single dot for `WIP` and nothing for any other
  status. Four colours on every row is a legend you have to learn, sitting where you least
  need it; one mark meaning "needs more", with absence meaning nothing to say, is
  glanceable without a key. This is what keeps WIP *ambient* — a saved search is opt-in, and
  "I forgot this folder was unfinished" is exactly the failure it cannot catch.
- **Last added** — tracked automatically. Makes WIP actionable: the folder list can be
  sorted by staleness, surfacing *"Ana — WIP — nothing new in 5 months"* instead of a
  flag you set once and forgot.
- **Notes** — free text, searchable.
- **Pinned** — pinned folders float to the top of the sidebar.

Folders nest arbitrarily. Every folder contributes its title as a tag automatically —
that is not editable and not removable.

### Folder operations

Folders are created, renamed, moved and deleted from inside the app. **None of these
touch a file.** Every one is a row update, which is what makes them instant, safely
undoable, and incapable of leaving the database describing something the disk contradicts:

- **Create** — one record, with an optional archetype.
- **Rename** — one column. There is one name and nothing derived from it.
- **Move** — dragging a folder onto another, or a menu action: `parent_id`, plus an
  effective-tag rebuild for the subtree, because inherited tags are recomputed from the
  new ancestry.
- **Delete** — the folder's record goes, and its items go to `.ggallery/trash/`. Never a
  hard delete.

Items move between folders the same way: drag onto a sidebar folder, a menu action, or a
triage hotkey. A move is a `folder_id` update plus a tag-cache rebuild for that item —
the file itself never moves, because its location is derived from its own uuid.

**All of these are journalled** so `Ctrl+Z` reaches them once the replayer lands. Renames
of *files* remain the exception — see §10.

### Folder names

**A folder has one name, and nothing constrains it.** The title is what the user types.
It is stored and shown lowercase (PLAN decision 31), and beyond that it can hold anything
— slashes, colons, emoji, four hundred characters, the same title as a folder in another
branch. Only siblings must differ.

*(This section used to specify how a title was sanitised into a directory name:
forbidden characters, reserved device names, trailing dots, length caps, collision
suffixes, and what happened when a title sanitised to nothing. Decision 30 deleted the
directory, and every one of those rules with it. The reasoning that survives is the
original one — two operations behind what reads as one control is a bug — which now
costs nothing to honour.)*

### Item operations

Beyond moving and tagging, the operations any file manager is expected to have, and which
this one needs because filenames on disk are opaque UUIDs:

- **Delete** the selection to `.ggallery/trash/`. Available from the grid, not only from
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
keyboard binding *and* a visible control, plus a live count of what is selected. Every item
operation above acts on the whole selection.

**One visual state, not two.** Selection is a filled accent border. The shift-click anchor
is not drawn at all — it is invisible bookkeeping, and rendering it competes with selection
for the same meaning, which is what makes an inverted selection ambiguous. Keyboard focus
uses `:focus-visible`, so it never appears after a mouse click.

### Tags

Two shapes, one system:

- **Label** — has a key and a value. `instagram: @ana`. Searchable by key, by value, or
  by both.
- **Flag** — value only. `beach`, `blurry`.

Both live in the same table and are matched by the same query syntax. A label with an
empty value still exists and still renders — that is what makes archetype labels visible
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

### Clicking vocabulary

**Every chip is a query term** (PLAN decision 32). A folder in a breadcrumb, a label in
the folder band, a flag in the details panel — clicking any of them writes its term into
the search bar and shows the result. `path:people/ana`, `instagram:@ana`, `beach`.
Ctrl-click appends rather than replaces.

**Chips are not clickable until the search bar exists** — M3. A chip that filtered the
grid directly would be the second model this rule exists to prevent, and building one
first means building it twice.

The alternative — folders navigate, tags filter — is one click shorter for the commonest
case and costs a second model to learn, a second thing that can be wrong, and the answer
to *"why am I looking at these?"* being invisible. One rule means the bar is always the
explanation, clearing it is always the way back, and composing two terms is something the
user discovers rather than something that has to be built.

### Lowercase

**Titles, label keys, label values and flags are lowercase**, folded on the way in rather
than at display time, so what you see is what is stored. Notes, original filenames and
every other free-text field keep their case.

Matching was already case-insensitive; identity was not, so `Beach` and `beach` could
both exist, split one tag's items between them, and appear twice in every list. And since
a folder's title is inherited as a tag, leaving titles cased would keep that split alive
at the top of the tree.

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
archetype. Add the new label to them?"* Removing a field never deletes existing values
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

The layout below is **one proposal, drawn early**. M2.5 designs the interface from scratch
and may reject it. The two subsections that follow are different: they are requirements any
design must satisfy, not suggestions.

### Navigation roots — a requirement

**The library root is not a folder in the interface.** It exists in the database because
items at the top level need somewhere to belong, but presenting it as a node called
"Library" that everything nests under is wrong: it implies a container the user did not
create and cannot remove.

Three distinct things must be reachable, however a design chooses to express them:

- **Everything** — every item in the library, recursively, ignoring folder structure.
- **Sorting Box** — items in no folder at all. Not everything recursively; just what has
  not been filed yet.
- **The folder tree** — the folders the user actually made. **When there are none, it shows
  nothing.** Not a root node, not a placeholder branch.

Then **Favourites**, then the tree. All expressible already: no filter, `is:unsorted`,
`is:favorite`, and the tree itself.

**The Sorting Box is "no folder", not a place.** There is no `Sorting Box/` directory and
no root folder standing in for one — an item that has not been filed simply has no
`folder_id`, which is the same statement with nothing to keep in sync. The gesture of
dropping files in from Explorer is served by the watched `<root>/inbox/`, and anything
that arrives there without a destination lands here.

### Direct manipulation — a requirement

The workflow this app replaces is dragging files between two Explorer windows. **Moving
items into folders, and folders into folders, must be possible by direct manipulation** —
picking something up and putting it somewhere. An interface that can only move things
through a context menu is a regression over what the user does today, however tidy the menu
is.

The gesture is M2.5's to design. That it must exist is not.

Dragging *into* the window from Explorer already works. Dragging *out* to Explorer is a
separate, harder problem — clipboard copy (§1) covers that need for now.

```
┌────────────────────────────────────────────────────────────────────────┐
│ ◈ GGallery      ‹ People / Ana      ⌕ query            ─  □  ✕         │
├──────────┬──────────────────────────────────┬──────────────────────────┤
│   NAV    │ ▸ Ana ●WIP 2,481 items  ▦──● ☐  │ ▸ file.jpg      ⊡ ⊞ ▤ ⤢ ×│
│          ├──────────────────────────────────┤──────────────────────────┤
│Everything│                                  │                          │
│SortingBox│                                  │                          │
│Favourites│                                  │                          │
│          │           MEDIA GRID           │s│        THE PANE          │
│Pinned    │      (justified, virtualized)  │c│                          │
│ Ana      │                                │r│   one of three modes     │
│          │                                │u│                          │
│Folders   │                                │b│                          │
│ People   │                                │ │                          │
│  Ana   ● │                                │ │                          │
│ Places   │                                  │                          │
│Searches  │                                  │                          │
│Queues    │                                  │                          │
│ Pending⁷ │                                  │                          │
├──────────┤                                  │                          │
│ ⚙  ◍ 42  │                                  │                          │
└──────────┴──────────────────────────────────┴──────────────────────────┘
      ↑                    ↑                              ↑
 folds to 44px    folder band, collapsed     drag-resizable, fully closable
```

### Every band owns one job

The window had grown a bar holding the library name, its full path, index status, a scope
checkbox, the tile-size slider, an *Open pane* button and a hamburger — seven unrelated
things with no organising idea, sitting under a Windows title bar that repeated the app's
name. A control with no obvious home ended up there, which is what made "where does tile
size go" unanswerable.

Three bands, and nothing lives in one that belongs in another:

| Band | Owns |
| --- | --- |
| **Window bar** | The window and the app — mark, name, window controls. Search and the breadcrumb join it in M3. |
| **Folder band** | The current grid — what you are looking at, and the controls that change it. |
| **Navigation footer** | The app's own state — Settings, background work. Ambient and ignorable. |

**The window bar is the app's own**, not Windows'. Native decorations are off: the mark and
*GGallery* sit at the left, minimise / maximise / close at the right in Windows order, and
the rest is a drag region. Double-click maximises; dragging to a screen edge still snaps.

**Known cost:** Windows 11's Snap Layouts flyout, which appears when hovering the native
maximise button, does not appear for a custom one without hit-testing that button as
`HTMAXBUTTON` in Rust. Accepted deliberately — one window on one monitor, and edge-drag
snapping is unaffected.

**The library's name and path are not in the chrome at all.** One library, chosen once; a
machine-specific path in permanent chrome is noise. Both live in Settings.

**Navigation panel** — resident, ~200px, drag-resizable, folded away by a visible control.
Width and folded state remembered; **never summoned by a keypress**. Folded, it becomes a
44px icon strip that keeps queue badges on screen and every root a drop target.

**A footer pinned below the tree** holds Settings, background-job status, and — when
nothing is running — the library's totals, `41,236 items · 1.84 TB`. Separated by a
hairline so none of it scrolls away with the tree and none is mistaken for a destination:
Settings opens a dialog; it is not a place. All of it survives folding: in the 44px strip
the footer is the gear and the job indicator alone.

*(The totals arrived with the drawing in M2.8, and they are the app's own state rather than
the grid's — the whole library, always the same number, not the count of whatever you have
open. That is what keeps them clear of the folder band's counts, which change as you
navigate. Total size is not computed anywhere yet.)*

Groups, in order: **Library** (Everything, Sorting Box, Favourites — above the tree, never
nodes in it), **Pinned**, **Folders**, **Saved searches**, **Queues** (Pending Review,
Trash, each with a count badge).

The Sorting Box is a *library root*, not a queue: it is a state rather than a folder, so
it belongs with Everything and Favourites rather than in a group of folders. The count
badge it would have had in Queues sits on that row instead.

**The grid's footer is status, not instruction.** It holds what is selected, and the
count is right-aligned — the edge a count belongs on, and where the eye already goes for
a total. *(It first held "right click for more" at the left. That is a tutorial, and a
tutorial in permanent chrome is a line you read once and then look past forever, taking
up the one place a live count could have been.)*

Pinned folders live in their own group above the tree rather than floating within it — so
favouriting something never reorders the tree, and the row you reach for stays where it was.

Folders accept drops. Right-click for new folder, rename, edit tags, set cover, set status.
A single dot marks `WIP` and nothing else; see §1 *Folders*.

**A row is selected by a filled rounded surface, not a border** — accent-tinted background
with accent text. Hover is the same rounded rectangle in neutral, one step lighter than the
panel, so the two are never confused and hovering the selected row still reads as selected.
See decision 26: a border suits a tile, where media fills the frame; a row has no frame and
a border round text is a box, not a state.

**Timeline scrubber** — a thin strip down the right edge of the grid. Dragging it jumps to
that point in the sort order. At 40k+ items this is the difference between a browsable
library and an endless scroll.

**No labels and no date.** Not a column of years down the strip, and not a date that
follows the thumb either — where the thumb sits *is* the information, and everything read
in under a second competes with the thing you are actually looking at. The scrubber is part
of the grid's own width — the bar beneath it must account for it rather than running
underneath.

*(M2.5a.1 removed the year column and kept a date on the thumb; M2.5a.2 removed that too.)*

**There is exactly one scrollbar.** The scrubber *is* the scroll affordance — the native
scrollbar is hidden with `scrollbar-width: none` while the scroll container stays fully
functional (wheel, keyboard, and programmatic scrolling all work unchanged). Showing both
is redundant and looks unfinished.

**Panels are resizable.** The navigation panel and the pane both have drag handles, with a
sensible minimum width. Widths persist between sessions alongside window geometry.
Double-clicking a handle resets that panel to its default width.

**One width for the pane, and it is not in Settings.** Both were tried in M2.5a and both
were wrong. A width per pane mode meant switching mode moved the split under you, which
reads as the window losing its place rather than as the app being helpful. And a slider in
Settings is a number to type at something you have already got right with the mouse —
dragging the edge *is* the control, and it is visible, which is all decision 23 asks.

**The native context menu is suppressed everywhere.** Right-click opens the app's own menu
appropriate to what was clicked — a folder, an item, a selection, or empty space. A
WebView's default menu appearing in a desktop app is a bug, not a placeholder.

**Settings is one dialog with a section list down the left**, the convention every desktop
app of this shape uses. Not one dialog per subject: M2.5a.2 found archetypes, statuses and
tags each opening their own dialog that replaced the last, so going from one to another
meant closing back to Settings and the backdrop flashed between them. A section list makes
the whole of Settings visible at once, which is the only way you discover the parts you
were not already looking for. Adding a subject adds a row, never a window.

*(Citation, per §Prior art: `sidebar-13` in the shadcn registry — "a sidebar in a
dialog" — is the same shape, `Dialog > Sidebar(collapsible="none") + content pane`, and
was checked against this section in M2.5a.3. The registry's own `Sidebar` primitive was
not adopted for the row list itself — it would pull in `SidebarProvider` and its
width/mobile-sheet machinery to replace a handful of plain buttons. See
[ENGINEERING-NOTES.md](ENGINEERING-NOTES.md#shadcnui--audit-vs-adopt-m25a3).)*

**Folder band** — a collapsed strip above the grid, and the only chrome scoped to what the
grid is showing. Closed, it is one line: chevron, title, status, counts on the left, and on
the right **every control that changes the grid** — scope, sort, layout, tile size, pin,
and an overflow menu for the folder's own actions. Those were in the window bar, which owns
the window, not the grid.

**Scope is a two-segment control**, *All items* / *Here only*, not one button that toggles
its own label. **Sort** offers captured date, added date, size, duration and random (§*Grid*);
**layout** offers justified rows and uniform grid.

*(M2.8 filled this strip out from three controls to six, taking the drawing as-is. The band
is the only chrome directly above what you are looking at, so its density is the thing to
watch — if it stops reading as one line, the overflow menu is where the rarest of these go,
and that is an M2.9 question rather than a reason to hold any of them back now.)*

Clicking expands it to the cover, archetype labels edited in place, tags and notes.

Expanded state is **global and remembered**, not per folder — it sits with panel widths and
window geometry, never in the database. Per-folder state would reflow the grid every time
you changed folder, and it is state nobody would curate.

#### The expanded band is identity, not a form

The first build of it was a data-entry form: four invitations to add something, an empty
notes box as the heaviest element on screen, and roughly 330px of vertical space conveying
nothing about a folder with nothing set. In a viewer-first app that is a third of the grid
spent on an empty form. The rules that follow all come from that.

- **Counts appear once**, in the header, in prose. The first build printed them twice in
  two different phrasings, the second in mono — which is reserved for paths, hashes and
  data, so a sentence set in it reads like debug output.
- **Status renders only when it is not `Active`.** Same rule the tree already follows:
  absence means nothing to say. A permanent `Active` chip is a legend for the default.
- **Weight follows importance.** The title identifies what you are looking at and should be
  the largest thing in the band. Notes are the least-used field and were the largest.
- **Notes are one line that grows on focus**, never a reserved box.
- **Labels and tags get a row each**, headed `LABELS` and `TAGS`, with their add controls
  at the end of their own row. *(M2.5c merged them into one row, on the grounds that two
  rows read as a data-entry form; M2.8 separated them again from the drawing, on the
  grounds that a folder carrying many of both is unscannable in one flow. Both are true —
  the merge was right for the empty folder, the split is right for the full one, and the
  full one is what the app is for. The empty-folder cost is real and is what M2.9 should
  look at first.)* **The same rule applies to an item's details panel**, which carries the
  same two kinds of thing and was merged in the same pass.
- **Applying an archetype is a once-per-folder setup action** and belongs in the folder's
  context menu, not as a standing button competing with content.
- **Favourite is a header control among the others**, not the heaviest thing in the band
  parked in the far corner.
- **Every folder shows its ancestry as a breadcrumb**, the same one the item details panel
  uses, the folder itself included as the last crumb — always at least one entry, since a
  top-level folder still sits somewhere, even if the answer is "at the top". Every crumb
  is a query term. Without it the band names a folder and says nothing about where it
  sits, which matters more here than for an item because the answer is also where the
  folder's inherited labels come from — and those are rendered greyed in the same chip
  row, so their origin has to be visible somewhere.
- **A folder-name tag is never rendered as a tag**, in this row or in an item's. Every
  folder auto-tags itself with its own title (§*Tags*), and that tag is inherited by
  everything beneath it — showing it as a chip too would repeat the breadcrumb a second
  time in tag shape. A manual tag that happens to share the same text is a deliberate
  choice, not the folder leaking through, and still renders.

**Design against the full case** — an archetype with five labels, eight tags and a real
note — and let the empty one be that band with things missing. It must look right with **no
archetype at all**, which is the default and commonest state; empty means the cover, the
counts and one *＋ add label* control, not a row of blank labels. The empty band should cost
around 140px, not 330.

**Grid** — justified rows, sized by a slider. Video items show a duration badge and
scrub through their sprite strip on hover. Selection is click, shift-click for range,
ctrl-click to toggle, drag for marquee. Sort by captured date, added date, size,
duration, or random.

**Subfolders are not shown in the grid.** The grid is media. Structure lives in the
navigation panel and the folder pane, and mixing folder tiles into a media grid makes both
worse — you cannot scan pictures past interruptions, and folders are easier to hit in a list.

### The pane

The right half of the split, and the single most reused surface in the app. Drag-resizable,
**fully closable**, one remembered width across every mode.

**One header, ending in maximise and close.** Each mode fills the rest of it with whatever
names what it is showing — for Preview that is the item's filename and size. Once there is
more than one mode to choose between, the switcher joins maximise and close as **three icon
buttons in that same group**, not a labelled tab row: the header's job is naming the item,
and three words of chrome is the widest thing that could take the space away from it. While
Preview was the only mode, a tab saying "Preview" was a label pretending to be a control, so
there is none.

**The mode buttons are always visible, and Preview always has something to show.** With
nothing selected it renders its empty state rather than disappearing — switching to Preview
is never a dead end, and the pane never changes shape because a selection was cleared.

**Closed, the pane folds to a strip of the three mode icons**, exactly as the navigation
panel folds to its icon rail. Clicking one opens the pane in that mode. There is no *Open
pane* button in the window bar: a control that exists only while a panel is closed is chrome
that has to live somewhere, and the panel's own edge is where it belongs.

**There is no theatre view.** Full-window is the pane maximised — one control, one state,
no transition to design and no scroll position to restore.

**Full-window is an arrow, and it sits at the left of the header**, pointing left to
expand and right to restore. Two reasons it is not with maximise and close on the right:
the arrow describes the direction the pane will actually travel, which only reads if the
control is on the edge the pane grows from, and it belongs beside *Details* — the other
control that changes how much of the pane you see — rather than beside the ones that
change *whether* the pane is there. *(It was first drawn as a generic fill-the-window
glyph on the right, which named a state rather than an action and pointed nowhere.)*

#### Preview mode

The selected item, fit to the pane. **Splits into N panes**, which is what makes it the
only comparison surface the app needs:

- *Images* — scroll to zoom, drag to pan, **the point under the cursor staying fixed** as
  the zoom changes. *(Specified first as the centre of the visible area, corrected in use:
  the centre is the right anchor for a keyboard zoom, and the wrong one for a wheel, which
  has already told you where you are looking.)* **No zoom UI at fit** — no fit button, no 1:1 button, nothing on
  screen. Once zoom leaves fit, a single small percentage readout appears in a corner of
  the viewer; clicking it returns to fit. One control, absent by default, and it doubles
  as the discoverable form of the double-click-to-fit gesture.

  *(The original rule banned a zoom toolbar outright: a permanent strip of chrome under
  every photograph competed with the photograph. That objection holds only for something
  permanent — a control that is absent until zoom actually leaves fit does not compete
  with anything, because at fit there is nothing there.)*
- *Video* — play/pause, scrub, frame-step, speed, **loop on by default**, and volume that
  persists between items.
- Chevrons and arrows move through the current filter, in the grid's order. A filmstrip
  shows position and allows jumping. Its height is dragged from its top edge and
  remembered; it has **no scrollbar**, and the chevrons are overlaid at either end.
  **No position counter** — the strip already shows where you are, and `6 / 15` is a number
  nobody acts on. *(M2.5c specified a scrollbar along the very bottom; the drawing has none
  and the drawing is the specification for appearance — M2.8.)*
- **The pane header is the details header.** A chevron, the filename, and dimensions ·
  size, opening **downwards** into duration, codec, dates, source URL, the item's folder
  breadcrumb (folder itself last, per §2) and tags — inherited greyed, manual solid. An
  inherited tag that is one of those folders' own name is never shown a second time as a
  chip; a manual tag sharing the same text still is. The media gives way; the filmstrip
  does not move.

  *(M2.5a.1 reversed this. Details first had a strip of their own above the filmstrip that
  grew upward, which left the pane with two headers and a band of chrome between the media
  and the strip. A pane has one header, and naming what you are looking at is what a header
  is for — which also retired the "Preview" tab, a label wearing a control's clothes while
  it was the only mode. M2.5b's switcher returns to that slot.)*
- With nothing selected the pane shows an empty state. Folder identity belongs to the band.

Multi-pane preview is one mechanism with three uses: **compression review** (M6) and
**duplicate comparison** (M7) are two panes with synced pan, zoom and a shared timeline;
**multi-view** (M10) is up to twelve, all playing, looping, muted, with one pane soloing
audio on click. Layout adapts — 2 side by side, 3–4 as 2×2, 5–6 as 3×2, 7–9 as 3×3, 10–12
as 4×3.

Multi-view still carries a real performance risk and is still measured before it is built:
twelve concurrent video elements can exhaust hardware decode sessions and fall back silently
to software. If the honest number is six, the cap is six, and panes past it show a poster
frame with a click-to-play control rather than stuttering.

#### Grid mode

A second grid, scoped anywhere in the library, with its own sort and tile size. Two folders
side by side, or a query against a folder. **It accepts drops** — drag items in and they
move there.

#### Folders mode

Destination tiles, and the reason the two-Explorer-window workflow is beaten rather than
tied.

- **One flat field per level.** No sections, no reordering, no sorting by recency — a folder
  is where it was last time, which is what lets the drag become muscle memory.
- **Tiles** show cover, title and item count. During a drag the count previews the result:
  `610 → 616`.
- **Single click drills in**; the main grid does not move. **Double click** navigates the
  main grid there.
- **Breadcrumb and an Up button** at the top. Both are drop targets.
- **A filter box pinned to the bottom** searches title and path across the whole library,
  flat, ignoring wherever you had drilled to. Clearing it restores your position. Whenever
  the list is flat, parent paths render under titles so two folders with the same name are
  distinguishable.
- **A *＋ New folder in ‹parent›* tile is always present**, and when nothing matches what you
  typed, a *Create "roo" in Trips* row appears. This is the inline folder creation §4 needs,
  as a visible control rather than a keystroke.
- **Dragging a folder onto a tile nests it.** A tile that would become its own descendant
  refuses visibly rather than silently doing nothing.

### Drops

Three targets: **folder tiles**, **tree rows**, and **the pane in Grid mode**.

**Nothing appears or rearranges mid-drag** — no dock sliding in, no automatic mode switch.
Targets that were on screen when you picked something up are the targets when you put it
down. The single exception is **spring-loading**: hovering a tree row or folder tile opens
it, so a nested destination can be reached without setting it up first.

**Every drop ends in a toast** naming the destination with an Undo button, and the
destination's count ticks up.

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

**An item with no folder.** Not a directory and not a special folder record — unfiled is a
state, so anything else would be a second way of saying it that could then disagree.
Items appear from:

- The app's **Add Files** picker (`Ctrl+O`)
- **Dragging from Explorer** onto the window
- **Downloads** (M5)
- **Dropping files into `<root>/inbox/`** in Explorer — the watcher picks them up,
  renames and shards them, and the file leaves `inbox/` as it is indexed. This is the
  place on disk the user is meant to put things: `files/` is sharded by uuid, so putting
  something there by hand means nothing.
- **Dropping files at the library root**, which is swept into `inbox/` and then treated
  identically. Muscle memory says the library folder is where media goes, and a file that
  sits at the root doing nothing is a silent failure — so the root behaves like the inbox
  rather than like a place.

**The library root is a hot zone, and that has to be said plainly.** Anything appearing at
the top level that is not `.ggallery/`, `files/`, `inbox/` or a dotfile is moved into
`inbox/` — by the watcher while the app runs, and at startup for anything that arrived
while it was closed. A directory dropped there is taken in whole: its files are renamed
to uuids, and **it is not preserved as a folder**, because only a first import reads
structure out of directories. There is no undo for this. It is the right default — the
alternative is files quietly rotting at the root — but it means the library folder is not
somewhere to park something temporarily.

Files arriving at the root are renamed to UUIDs, hashed, thumbnailed, and checked against
existing content hashes. Exact duplicates of something already in the library are flagged on
arrival rather than after you have already sorted them.

Downloads land here too. Triage is finished when the root is empty.

### Triage without the keyboard

Triage below is written in keystrokes, and it must not require them. **The mouse path is the
ordinary window**: Sorting Box in the grid, the pane in Folders mode, drag onto a tile. No
mode to enter, no hotkeys to have assigned, nothing to learn first — the same surface used
for everything else, pointed at the queue.

The fullscreen culler is the accelerator for when you want it, not the only way in.

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

Compressed output is written to `.ggallery/pending/`. The original is untouched until you
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

**Compare view** — the pane in Preview mode, split into two. Not a screen of its own; see
§2 *The pane*.

- *Images* — synchronised pan and zoom, a 1:1 toggle, and a wipe-slider mode for A/B on a
  single image.
- *Video* — two players sharing one timeline. Play, pause, scrub and frame-step move
  both in lockstep. Frame-stepping is where compression artifacts actually show.
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

Perceptual hashing for images; sampled-frame hashing for video. Groups are surfaced in the
pane's split Preview mode — the same surface compression review uses — with resolution,
bitrate, size and date shown against each candidate.

Choosing a keeper merges the loser's manual tags into it before trashing, so you never
lose tagging work by deduplicating.

---

## 8. Cross-cutting

**Undo** — a persistent journal. `Ctrl+Z` works across restarts and treats a batch
operation as one step. This is what makes fast triage comfortable.

**Command palette** — `Ctrl+K`. Folders, saved searches, settings, and every action.

**Trash** — soft delete. Files move to `.ggallery/trash/` preserving their relative path;
the database row keeps a `deleted_at`. Restorable until purged.

**Nothing is keyboard-only.** Every action has a visible control; keys are a second path to
something already on screen. If an action can only be performed by knowing a shortcut, it is
not finished. Locked decision 23.

This is why **every destructive action ends in a toast naming what happened with an Undo
button** — `Ctrl+Z` alone is not a path to undo, and the toast is also what makes the
journal discoverable at all.

Controls that must exist visibly, not only as bindings: select all, invert, clear;
favourite; delete; reveal in Explorer; open with; copy file; copy path; blur; fold the
navigation panel; negate a query term. Right-click menus are complete, not a subset.

Keys remain for everything, destination hotkeys are user-assigned, and a `?` overlay lists
them — as an accelerator layer over a fully usable mouse interface.

**Colour** — exactly one accent hue carries selection, focus, the active tab, drop
acceptance and the panel drag handles, chosen by the user from a fixed set: Slate (default), Teal, Violet,
Rose, Moss, Amber. Fixed rather than free so every value is contrast-checked against the
same greys. Green and red are reserved for meaning — kept, saved, deleted, failed — and are
never the accent. Locked decision 24.

**Single instance** — a lock file in `.ggallery/` prevents two copies opening the same
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

Pointing the app at a library it has never seen renames every file to a UUID and moves it
into `files/`. It happens once, before the library is ever shown.

**The directory tree is read once, and becomes folders.** A first import is not simply a
bulk `inbox/` drop: every directory in the imported tree becomes a folder record with the
matching parentage, and each file is filed into the folder it was already in. Only files
that were loose at the top level land in the Sorting Box.

This is the one moment the app ever reads meaning out of a directory structure, and it is
worth doing because it is the only moment the meaning still exists. Sweeping everything
into the Sorting Box instead would discard, in a single irreversible step, the entire
organisation the user built before the app existed — and asking them to rebuild it by
hand afterwards is exactly the tedium the product exists to remove. *(This is not
folder-name **parsing**, which stays a non-goal: a directory becomes a folder with that
title, and nothing is inferred from how the title is written.)*

Directory names arrive lowercased like every other title (decision 31), and siblings that
collide once folded are merged rather than suffixed.

**It is part of the startup flow, not a dialog over the app.** The sequence is full-window
screens, in the same visual language as the folder picker — no modal floating above a grid
that is already loading thumbnails of files about to be renamed. Nothing is indexed, no
thumbnail is generated, and no `.ggallery/` content is written until the rename has run.
The library is normalised first, then opened.

```
  ┌──────────┐     ┌──────────┐     ┌──────────┐     ┌─────────┐
  │  Choose  │ ──▶ │  Review  │ ──▶ │ Progress │ ──▶ │ Gallery │
  │  folder  │ ◀── │          │     │          │     │         │
  └──────────┘ Cancel─────────┘     └──────────┘     └─────────┘
```

**Choose folder** — the existing picker.

**Review** — one screen, and the only one that asks anything:

- What was found: file count, total size, folder count, anything unreadable.
- What will happen, in one sentence: files are renamed to UUIDs and stored flat, the
  folder structure is kept as folders, and original names are kept and shown in each
  file's details.
- One checkbox: *I have a backup of this folder.* Nothing proceeds without it.
- **Cancel** returns to the folder picker. **Import** starts.

**Progress** — build the folders, then move and rename, then index, then thumbnails, as
one continuous readout. Verification runs here silently: a random sample is re-hashed and
counts are confirmed, surfaced only if it fails.

Then the gallery opens.

### Keep it short

The flow above is deliberately two screens. An import wizard that explains itself across
six panels reads as nervous, and a user who is asked to confirm four times stops reading by
the third.

Two checkboxes, both on the review screen: **that a backup exists**, because there is no
undo, and **dry run first**, which walks the whole import and reports what it would do
without touching a file. *(The second arrived with the drawing in M2.8. It is not a fourth
confirmation — it is the same offer M2.6's storage migration makes, on the other operation
that touches every file in the library at once.)*

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

**There is no *Normalise filenames* action any more.** It existed because an item's name
on disk could fall behind `<uuid>.<ext>` and need catching up. Since M2.6 an item's
location *is* a function of its uuid — it is sharded the moment it is indexed — so "not
yet renamed" is not a state that can exist. What is left is a different question: a file
in `files/` with no row, or a row whose file has gone missing. That is a reconcile pass,
not a rename, and it belongs with the integrity screen in M8.

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

`.ggallery/` is excluded from watching, and paths the app is itself mid-write on are
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
