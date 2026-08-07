# Where this came from

The interface design for GGallery, produced in Claude Design and copied here **verbatim**
on **7 August 2026**. This directory is the design's home now. Nothing outside it should
reach for the original.

| File | What it is |
| --- | --- |
| `GGallery.dc.html` | **The design.** Every screen, state and value. 2411 lines. |
| `support.js` | The Claude Design runtime that renders `.dc.html`. Generated, not authored — `// GENERATED from dc-runtime/src/*.ts — do not edit`. **Carries no design decisions.** |
| `.thumbnail` | Canvas preview image, kept only so the set is complete. |

Original project: `https://claude.ai/design/p/9e4a734b-856b-4e01-b72c-50ee81adaa83`

Re-fetching needs the `claude_design` MCP server (`https://api.anthropic.com/v1/design/mcp`)
and an interactive `/design-login`, which only the user can run. **It should not be
necessary.** These files are the
copy of record; if the design changes, replace them here in one commit and say what moved.

## Reading it

**The design lives in the HTML.** The markup from `<x-dc>` to `</x-dc>` is the layout;
the `<script type="text/x-dc" data-dc-script>` block at line 1489 holds the state, the
screen list, the accent definitions and every piece of sample data. Between them they are
the whole specification. `support.js` is a viewer — reading it teaches you about Claude
Design, not about GGallery.

Template syntax, so it reads correctly: `{{ x }}` interpolates, `<sc-if value="{{ x }}">`
is a conditional, `<sc-for list="{{ xs }}" as="x">` is a loop, and `style-hover="…"` is
the hover state. `hint-placeholder-count` and `hint-placeholder-val` are authoring hints
for the canvas and mean nothing.

## Opening it in a browser

It renders, but **only with a network connection** — it pulls React, ReactDOM and Babel
from `unpkg.com` at runtime, and its icons from `lucide-static`. Offline it will be blank.

That is a property of the mockup, not of the app, and it is the reason the source is the
authority here: reading the file needs nothing. If offline rendering turns out to matter,
the three UMD bundles can be vendored next to it — about 500KB — but that is a
convenience, not a requirement.

**Nothing in this directory ships.** No build step reads it, no shipped code references
it, and the `unpkg.com` fetches inside it must never be copied into the application —
`lucide-react` is already a dependency and is where icons come from.

## What it covers

Twelve screens, seven states and four accents. Beyond the built interface it draws
**Search (M3), Triage (M4), Downloads (M5), Pending Review (M6), Duplicates (M7), Storage
and Tags (M8), and Multi-View (M10)** — so most of it is specification for milestones that
have not started, not a restyling of what exists.

**First Import is not one of those** — it is built, and the drawing's version is a restyle
of a real screen.
