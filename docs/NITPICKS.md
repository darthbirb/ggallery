# Nitpicks

Where small interface complaints go between now and **M2.9**, so they stop having to
interrupt whatever milestone is being built.

Anything counts: a control in the wrong place, a word that reads oddly, spacing that
looks off, a state nobody designed, an animation that stutters, an icon that means the
wrong thing. It does not need to be justified when it is written down — *"this looks
wrong"* is a complete entry. Working out why is M2.9's job.

## How an item is resolved

Each one is asked a single question first: **is this an instance, or a class?**

*"The fill-window icon points nowhere"* is an instance. *"An icon should name the action,
not the state"* is the class underneath it, and the class is what stops the next four.
Every locked decision in [DECISIONS.md](DECISIONS.md) started as somebody's nitpick. An item
that turns out to be a class is written into [SPEC.md](../SPEC.md) or the decision list;
fixing only the instance is how the same complaint comes back wearing different clothes.

Then one of three outcomes, all of them legitimate:

- **Fix** — the build did not match the specification, or nothing had specified it.
- **Change the spec** — the build was right and the specification was wrong. Amend it,
  and say what changed the mind.
- **Won't do** — with the reason recorded, so it is not re-raised in six months.

## Open

- **Grid — no drag-marquee selection.** `state/selection.ts`'s own doc comment says
  rubber-band selection "arrives with direct manipulation in M2.5b," but DECISIONS.md's actual
  M2.5b scope line only names the pane's Grid/Folders modes, the three drop targets,
  spring-loading and inline folder creation — marquee-select is a separate gesture (drag
  from empty grid background to select a rectangle of tiles, versus dragging a tile to move
  it) that M2.5b did not build. Noticed while implementing M2.5b's actual drops.

<!--
Format, for reference — delete nothing from here, it is not a list of real items:

- **Folder band** — the count reads "12 items, 3 folders" but the grid shows 12 tiles and
  no folders, so the second number describes something not on screen.
-->

## Resolved

*(Filled in during M2.9: the item, the outcome, and the class it belonged to if it had
one.)*
