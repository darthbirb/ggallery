# Locked decisions

Numbered, and **the numbers never change** — code comments cite them by number. A decision
is reversed by rewriting it in place, not by renumbering around it.

Each says what is true now. Where a decision carries a warning, the warning is there
because the mistake was already made once.

---

1. **Folders are entities.** Title, optional archetype, labelled fields, free tags, cover
   image. Searchable and taggable in their own right.

2. **Tags are inherited live.** An item's effective tags are recomputed from its current
   location every time it moves. No accumulation, no drift.

3. **Two tag shapes.** Labels (`instagram: @ana`) and flags (`beach`).

4. **Archetypes are folder templates.** One pre-creates a set of empty labels that stay
   visible while unfilled.

5. **Filenames are UUIDv4** plus the real extension. The original filename is kept in the
   database as searchable metadata.

6. **The Sorting Box is "no folder", not a place.** An item with no `folder_id` is unfiled
   by definition. There is no `Sorting Box/` directory and no magic location. Files arrive
   via the app, Explorer drag-and-drop, downloads, or `<root>/inbox/`, and anything
   arriving without a destination lands there.

7. **Triage is fullscreen-first**, one item at a time with destination hotkeys. Grid
   multi-select is one keystroke away.

8. **Every compression is reviewed manually** before anything is replaced.

9. **Replaced originals go to trash**, purged on demand with a visible reclaimable-space
   figure.

10. **Folder views are recursive by default**, with a *here only* toggle.

11. **Nothing is written outside the app directory and the library root.** No registry, no
    `%APPDATA%`, no installer. The app runs from a USB stick.

    This governs the **shipped application at runtime**, not the build toolchain. Cargo and
    npm keep machine-wide caches by design; do not try to relocate them.

12. **Favourite is first-class, not a tag.** One key, a badge on the thumbnail, a permanent
    sidebar entry. Binary — no star ratings, no colour labels.

13. **Folders carry a status** — Active / WIP / Done / Archived — plus a tracked *last
    added* date, so WIP becomes a staleness-sorted to-do list rather than a label you
    forget you set.

14. **Filenames are opaque, so export exists.** Selecting items and exporting them
    reconstructs meaningful filenames into a chosen location.

15. **Thumbnails are WebP** (libwebp, lossy q78). AVIF encoded 41× slower and would not
    decode at all in the `image` crate on this platform, for a 12% size win.

16. **WebView2's data directory is redirected into the app directory.** Tauri defaults it
    to `%LOCALAPPDATA%\<bundle-id>\`, which silently breaks decision 11.

17. **Animated GIFs are video; nothing is converted at import.** GIF, WebP and APNG stay in
    their original format on disk. Converting to MP4 is a compression preset, reviewed like
    any other. Import never rewrites an original.

18. **There is no polish phase.** Every milestone ships finished. Deferred polish is
    abandoned polish.

    **`shadcn/ui` is the component source** — Radix for behaviour plus designed Tailwind
    defaults, copied into `src/components/ui/` and restyled against our own tokens. Radix
    alone is headless and ships no visual design; adopting it bare is how an interface ends
    up with correct behaviour and hand-rolled appearance. A bespoke layout does not require
    bespoke buttons.

19. **Renaming is a property of indexing, not a one-time event.** Files the app creates are
    born `<uuid>.<ext>`. Files arriving from outside are renamed as part of being indexed,
    silently and journalled. First import is the same operation run over a whole
    pre-existing library at once, with a dry run and a backup gate because the scale makes
    it dangerous. It is offered while opening an unimported library and never becomes a
    standing button.

20. **Anything that adds a query path is verified against a synthetic library at scale.**
    The working library is a few hundred files, so nothing will feel slow during
    development. A query that is instant over 198 rows can be catastrophic over 100k with
    joins — and the effective-tag cache, search and duplicate grouping are all exactly that
    shape. Keep a generator that produces a synthetic 100k-item library, and run the
    milestone's new queries against it before calling the milestone done.

    Scale problems surface as architecture, not as ordinary bugs. Finding one after four
    dependent milestones is the expensive way.

21. **The app ships with no domain vocabulary.** No seeded archetypes, no named field types,
    no folder-name conventions, nothing that assumes what the library is *of*. "Person",
    "instagram", "Place" and every example in these documents illustrate how someone might
    use the app; they are never strings in the product. Archetypes, labels, flags and status
    values are created by the user, starting from empty.

    Broken twice — a migration seeding a Person archetype with social-platform fields, and a
    "parse folder names" action built around one naming habit. Both are one user's current
    data shape promoted into product behaviour. **When a feature only makes sense if you
    already know what the user collects, it does not belong in the app.**

22. **Every noun needs a full lifecycle, written down as operations.** If the specs describe
    something the user can have — a folder, a tag, an archetype, a saved search, a status
    value — they must also describe creating, renaming and removing it, as capabilities in
    their own right. Describing an operation only as an entry in some future context menu is
    how folder creation went missing for nine milestones: the menu item was specced twice
    and the capability never once.

23. **Nothing is keyboard-only.** Every action has a visible control. Keys are a second path
    to something already on screen, never the only path.

    Two consequences: **every destructive action ends in a toast naming what happened, with
    an Undo button** — `Ctrl+Z` alone is not a discoverable path to a journal. And triage
    needs its mouse path — the ordinary window, Sorting Box in the grid, folder pane open.
    Hotkeys stay; they stop being the only route.

    Right-click menus are complete, not a subset.

24. **One accent, chosen from a fixed set.** Exactly one hue carries selection, focus, the
    active tab, drop acceptance, the scrubber position and the panel drag handles. The user
    picks from a short list — **Azure (default), Steel, Teal, Indigo** — so every value is
    contrast-checked against the same greys rather than trusted to a free colour picker.

    Each accent carries two tint levels, **15% and 26%**, which are the fill behind every
    selected, active and drop-accepting surface. One fill, not a per-component guess.

    Green and red stay reserved for meaning — kept, saved, deleted, failed — and are never
    the accent. Amber means unfinished, not wrong. Swap via a `data-accent` attribute, which
    is scoped to *any* element carrying it rather than to `:root` alone, so a swatch in
    Settings can paint itself in a hue that is not the active one.

25. **Controls are sized to be hit and seen.** The scales below were inventoried from the
    drawing — every `font-size` and every control height across all twelve screens — not
    chosen here.

    **Type — nine sizes, and nothing else exists.** In pixels, because `12px` means the same
    in `--font-ui` and `--font-mono`; the family is chosen separately. Declared as
    `--text-10` … `--text-28` in `styles/index.css`.

    | | Sans (`--font-ui`) | Mono (`--font-mono`) |
    | --- | --- | --- |
    | `10` | — | section headings, `600`, `.12em` |
    | `11` | the mark's glyph only | badges, shortcut column, metadata, group headings (`600`, `.14em`) |
    | `12` | hints and sub-labels | **the default** — paths, names, counts, durations |
    | `13` | **the default** — control labels, menu rows | dense data tables |
    | `14` | body copy, rows, large controls | two readouts only |
    | `15` | a pane header inside a dialog | — |
    | `16` | screen and band titles, `600` | — |
    | `26` | full-window headings and stat values, `600`, `-.02em` | — |
    | `28` | the largest stat value, `600` | — |

    **Heights — ten values in five families**, and the family is what fixes the number:

    - **A control with a surface**: `26` (chip height — dashed ＋ buttons, the status chip) ·
      `28` small · `32` default · `38` large. Square icon buttons are `32` and `38`.
    - **A sub-control inside another control**, transparent until hover: `16` (a field's
      clear ×) · `18` (a chip's remove ×) · `20` (a row's chevron, `+`, `⋯`).
    - **A segment** of a segmented control, or a toast's dismiss: `24`.
    - **A menu row**, and the triage hotkey buttons sharing its height: `30`.
    - **The fullscreen culler's hotkeys**: `34`.

    - The glyph is **smaller beside a label than alone** — `16px` in a labelled 32px button,
      `18px` in a square one — which keeps a label and a glyph reading as one centred group
      rather than a glyph with text after it. `15px` at 28, `12px` at 26, `22px` at square 38.
    - **Every button with a surface has a background and a border at rest.** The one variant
      without a surface is `subtle`, which is not a ghost button: it is the sub-control
      family above, and it hovers to a translucent white overlay rather than to
      `--color-hover`, so it reads the same on a plain row and on an accent-tinted one.
    - **Anything clickable shows `cursor: pointer`** — rows, tiles, tabs, chips, swatches,
      drag handles (which get the resize cursor for their axis). Implement as one global
      rule on `<button>`, not a class per call site, or it will be as complete as the last
      person's memory of it. **Scrollbars are the exception**; the scrubber is not, because
      it is clicked to jump as well as dragged.

    **The enforcement point is `components/ui/button.tsx`'s `cva` variants.** The point of
    this decision is not the specific numbers — it is that a height never appears because
    somebody needed one and reached for it locally.

26. **Selection is one treatment per shape; focus rings are for keyboards.**

    On a **tile**: an accent border, a full-bleed accent wash *above* the media, and a tick
    badge — three marks for one meaning, always together, never independently. On a **row**:
    a filled rounded surface, accent-tinted background and accent text. A border is wrong on
    a row, which has no frame, and draws a box around text.

    Hover is the same shape in neutral, one step lighter, so hovering a selected row is
    visibly not the same as selecting it.

    The shift-click anchor is **not rendered** — it is invisible bookkeeping, and drawing it
    puts two competing meanings on one tile. Keyboard focus uses `:focus-visible` only, so
    it never appears after a mouse click.

27. **Motion is short, functional and interruptible.** Anything that changes size or
    position animates — panel folds, band expansion, details opening, filmstrip resize —
    because a panel that teleports makes you re-find your place. Nothing decorative
    animates: no entrance effects, no staggered lists, no spring overshoot.

    One scale: `120ms` for hover and colour, `180ms` for layout, `ease-out` for both.
    Animate `transform` and `opacity`; animating `height` or `width` on a surface containing
    the grid is a per-frame relayout and costs more than the animation is worth. Honour
    `prefers-reduced-motion`.

    **A single icon that rotates beats a conditional pair of icons.**

28. **Every band owns one job, and the window bar is ours.** Three bands, each with an owner:

    | Band | Owns |
    | --- | --- |
    | **Window bar** | The window and the app — mark, name, search, breadcrumb, window controls |
    | **Folder band** | The current grid, and every control that changes it |
    | **Navigation footer** | The app's own state — Settings, background work, library totals |

    Native decorations are off. **A control that fits none of the three is a sign the control
    is wrong, not that a bar needs another slot.** Horizontal chrome accretes otherwise: the
    first shell grew a bar holding the library name, its full path, index status, a scope
    checkbox, a tile-size slider, an *Open pane* button and a hamburger.

    **A panel's reopen control lives on the panel's own edge**, never in a bar. The
    navigation panel folds to a 44px icon rail; the pane folds to a strip of its three mode
    icons.

29. **The app has a mark, and the mark is not the accent.** It reads at 16–20px in the
    window bar and doubles as the Windows `.ico`. It stays neutral: the accent is user-chosen
    and changes per session, and an identity that changes colour with a preference is not an
    identity.

30. **Folders are data. Files are stored flat.** The hierarchy — parentage, titles, order —
    lives in the database and nowhere else. On disk every file sits at
    `<root>/files/<first two hex chars of uuid>/<uuid>.<ext>`, sharded 256 ways so no
    directory holds 100k entries.

    A move is `UPDATE folder SET parent_id`, a rename is one column, undo is one row. None of
    `MAX_PATH`, forbidden characters, reserved device names or case-insensitive sibling
    collisions reach a folder title at all. **Everything the filesystem used to enforce left
    the codebase with this decision — do not reintroduce a sanitiser.**

    **What it gives up is the redundant copy**, and two things buy it back: `library.jsonl`
    is the rebuild path rather than a convenience, and `.ggallery/backups/` keeps rolling
    database copies. Both are load-bearing.

    **New files arrive through `<root>/inbox/`**, which is watched. Dropping a file into
    `files/` by hand is meaningless.

31. **Everything the tag system stores is lowercase.** Folder titles, label keys, label
    values and flags are folded on the way in — typing `Beach` stores and displays `beach`.
    Notes, original filenames and every other free-text field are untouched.

    Case-insensitive *matching* is not enough; **identity** has to be case-insensitive too,
    or `Beach` and `beach` both exist, mean the same thing, and count separately. Since a
    folder title is inherited as a tag, cased titles would keep that alive at the top of the
    tree.

    This is data, not chrome. Interface strings are Title Case; anything that came out of the
    database renders exactly as stored.

32. **Every chip is a query term.** Clicking a folder, label or tag chip anywhere — the
    breadcrumb, the folder band, the details panel — writes its term into the search bar and
    shows the result. `path:people/ana`, `instagram:@ana`, `beach`. Ctrl-click adds to what
    is there rather than replacing it.

    One model: the bar always shows why the grid holds what it holds, and going back is
    clearing it. **Needs the search bar to exist** — a chip that filters the grid directly is
    the second model this decision exists to prevent.
