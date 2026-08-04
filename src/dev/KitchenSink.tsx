/**
 * Every primitive in every state, on one page. Dev-only — see App.tsx and
 * docs/STRUCTURE.md.
 *
 * Nothing here is a real control: hover and focus are forced with the
 * `force-hover:`/`force-focus` helpers in styles/index.css rather than
 * relied on from an actual cursor, so the whole page is correct in a single
 * screenshot. The values duplicated below (hover colours, row states) are
 * read from the components they demonstrate — button.tsx, badge.tsx,
 * Nav.tsx — not invented; if those change, this page drifts and should be
 * re-checked against them, the same as any other consumer.
 */

import { ChevronRight, Image as ImageIcon, LayoutGrid, Minus, Square, Star, X } from "lucide-react";
import type { ReactNode } from "react";

import { Chip } from "../components/Chip";
import { Mark } from "../components/Mark";
import { WindowBar } from "../components/WindowBar";
import { Badge } from "../components/ui/badge";
import { Button, IconButton } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { Input, PillInput, Textarea } from "../components/ui/input";
import { Label } from "../components/ui/label";
import { Separator } from "../components/ui/separator";
import { Slider } from "../components/ui/slider";
import { cn } from "../lib/utils";
import { ACCENTS } from "../state/ui";

const VARIANTS = ["default", "accent", "danger", "good"] as const;
type Variant = (typeof VARIANTS)[number];

/** The hover-target colours each button variant defines in button.tsx,
 *  applied statically via `force-hover:` instead of relying on `:hover`. */
const BUTTON_FORCE_HOVER: Record<Variant, string> = {
  default: "force-hover:border-fg-dim force-hover:bg-hover force-hover:text-fg",
  accent: "force-hover:bg-accent/25 force-hover:border-accent",
  danger: "force-hover:border-danger/70 force-hover:bg-danger/22",
  good: "force-hover:border-good/70 force-hover:bg-good/22",
};

const STATES = ["Rest", "Hover", "Active", "Disabled"] as const;

export function KitchenSink() {
  return (
    <div className="h-full overflow-y-auto bg-ground p-8 text-fg">
      <div className="mx-auto flex max-w-[1100px] flex-col gap-10 pb-16">
        <header>
          <h1 className="text-[20px] font-semibold">Kitchen sink</h1>
          <p className="mt-1 text-fg-mid">
            Every primitive, every state, forced rather than hovered. Dev-only —
            see <span className="font-mono">docs/STRUCTURE.md</span>.
          </p>
        </header>

        <Section title="Window bar — decision 28, decorations off">
          <div className="max-w-[560px] overflow-hidden rounded-[6px] border border-line">
            <WindowBar />
            <div className="flex h-16 items-center justify-center bg-ground text-fg-dim">
              rest of the window
            </div>
          </div>

          <p className="text-fg-dim">Caption button hover — minimise, maximise, close</p>
          <div className="flex h-8 w-fit items-stretch overflow-hidden rounded-[4px] border border-line">
            <div className="force-hover flex w-11 items-center justify-center text-fg-mid force-hover:bg-hover force-hover:text-fg">
              <Minus className="size-3.5" />
            </div>
            <div className="force-hover flex w-11 items-center justify-center text-fg-mid force-hover:bg-hover force-hover:text-fg">
              <Square className="size-3" />
            </div>
            {/* The one caption button that departs from the neutral hover —
                red, matching every Windows title bar's close button. */}
            <div className="force-hover flex w-11 items-center justify-center text-fg-mid force-hover:bg-danger force-hover:text-white">
              <X className="size-4" />
            </div>
          </div>
        </Section>

        <Section title="Buttons — sizes × states (variant: default)">
          <Grid columns={STATES.length + 1}>
            <Cell />
            {STATES.map((state) => (
              <HeadCell key={state}>{state}</HeadCell>
            ))}
            {(["sm", "default", "lg"] as const).map((size) => (
              <Row key={size}>
                <HeadCell>{size}</HeadCell>
                <Cell>
                  <Button size={size}>Button</Button>
                </Cell>
                <Cell>
                  <div className="force-hover">
                    <Button size={size} className={BUTTON_FORCE_HOVER.default}>
                      Button
                    </Button>
                  </div>
                </Cell>
                <Cell>
                  <Button size={size} active>
                    Button
                  </Button>
                </Cell>
                <Cell>
                  <Button size={size} disabled>
                    Button
                  </Button>
                </Cell>
              </Row>
            ))}
          </Grid>
        </Section>

        <Section title="Buttons — variants × states (size: default)">
          <Grid columns={STATES.length + 1}>
            <Cell />
            {STATES.map((state) => (
              <HeadCell key={state}>{state}</HeadCell>
            ))}
            {VARIANTS.map((variant) => (
              <Row key={variant}>
                <HeadCell>{variant}</HeadCell>
                <Cell>
                  <Button variant={variant}>Button</Button>
                </Cell>
                <Cell>
                  <div className="force-hover">
                    <Button variant={variant} className={BUTTON_FORCE_HOVER[variant]}>
                      Button
                    </Button>
                  </div>
                </Cell>
                <Cell>
                  <Button variant={variant} active>
                    Button
                  </Button>
                </Cell>
                <Cell>
                  <Button variant={variant} disabled>
                    Button
                  </Button>
                </Cell>
              </Row>
            ))}
          </Grid>
        </Section>

        <Section title="Icon buttons — never below 32×32">
          <div className="flex flex-wrap items-center gap-3">
            <IconButton aria-label="Star">
              <Star />
            </IconButton>
            <div className="force-hover">
              <IconButton aria-label="Star" className={BUTTON_FORCE_HOVER.default}>
                <Star />
              </IconButton>
            </div>
            <IconButton aria-label="Star" active>
              <Star />
            </IconButton>
            <IconButton aria-label="Star" disabled>
              <Star />
            </IconButton>
            <IconButton aria-label="Star" size="icon-lg">
              <Star />
            </IconButton>
            <div className="force-focus">
              <IconButton aria-label="Star">
                <Star />
              </IconButton>
            </div>
          </div>
        </Section>

        <Section title="Rows — selected, hovered, both">
          {/* The exact classes Nav.tsx's ROW_IDLE/ROW_ACTIVE define, forced
              rather than hovered. */}
          <div className="flex max-w-[280px] flex-col gap-0.5 rounded-[6px] border border-line bg-panel p-1">
            <div className="flex h-8 items-center rounded-[4px] px-2.5 text-fg-mid">
              Idle
            </div>
            <div className="force-hover flex h-8 items-center rounded-[4px] px-2.5 text-fg-mid force-hover:bg-hover force-hover:text-fg">
              Hovered
            </div>
            <div className="flex h-8 items-center rounded-[4px] bg-accent/15 px-2.5 text-accent">
              Selected
            </div>
            <div className="force-hover flex h-8 items-center rounded-[4px] bg-accent/15 px-2.5 text-accent force-hover:bg-accent/25">
              Selected + hovered
            </div>
          </div>
        </Section>

        <Section title="Folder band — collapsed, every scope">
          {/* The exact row shape FolderBand.tsx renders — a scope with no
              folder identity gets a plain label and no chevron; a folder
              gets one, plus its status chip only when it is not Active. */}
          <div className="flex max-w-[720px] flex-col divide-y divide-line-soft rounded-[6px] border border-line bg-panel">
            <BandRow label="Everything" hasChevron={false} counts="42,481 items" />
            <BandRow
              label="Ana"
              hasChevron
              status={{ label: "WIP", colour: "#eab308" }}
              counts="2,481 here · 2,481 below"
              thisFolderOnly
              favourite
            />
            <BandRow
              label="Trips"
              hasChevron
              counts="12 here · 12 below"
              thisFolderOnly
              favourite
            />
          </div>
        </Section>

        <Section title="Folder band — expanded, empty vs. the full case">
          <p className="text-fg-dim">
            No archetype at all is the default state — cover, counts and one
            ＋ add field control, ~140px. The full case (five fields, eight
            tags, a real note) is what it grows into, never what it starts as.
          </p>
          <div className="flex max-w-[720px] flex-col gap-4">
            <div className="rounded-[6px] border border-line bg-panel">
              <BandRow label="New folder" hasChevron expanded counts="0 here" />
              <BandExpanded />
            </div>
            <div className="rounded-[6px] border border-line bg-panel">
              <BandRow
                label="Ana"
                hasChevron
                expanded
                status={{ label: "WIP", colour: "#eab308" }}
                counts="2,481 here · 2,481 below · 3 subfolders · added today"
                thisFolderOnly
                favourite
              />
              <BandExpanded
                fields={[
                  ["instagram", "@ana"],
                  ["tiktok", "@ana.trips"],
                  ["youtube", ""],
                  ["city", "Lisbon"],
                  ["born", "1994"],
                ]}
                tags={["beach", "portrait", "summer", "family", "2024", "favourites", "friends", "golden hour"]}
                note="Shoots best in the morning light — check the harbour folder for the boat trip set before deleting anything."
              />
            </div>
          </div>
        </Section>

        <Section title="Chips">
          <div className="flex flex-wrap items-center gap-2">
            <Chip>Manual tag</Chip>
            <Chip muted>Inherited tag</Chip>
            <Chip colour="#dc8199">Status colour</Chip>
            <Chip onRemove={() => {}}>Removable</Chip>
          </div>
        </Section>

        <Section title="Badges">
          <div className="flex flex-wrap items-center gap-2">
            <Badge>12</Badge>
            <Badge variant="accent">12</Badge>
            <Badge variant="danger">12</Badge>
            <Badge variant="bare">12</Badge>
          </div>
        </Section>

        <Section title="Inputs">
          <div className="flex max-w-[420px] flex-col gap-3">
            <Field label="Rest">
              <Input placeholder="Placeholder" />
            </Field>
            <Field label="Focus (forced)">
              <div className="force-focus">
                <Input
                  defaultValue="Typed text"
                  className="force-focus:border-accent-d"
                />
              </div>
            </Field>
            <Field label="Disabled">
              <Input defaultValue="Can't touch this" disabled />
            </Field>
            <Field label="Textarea">
              <Textarea defaultValue="A note, wrapping across a couple of lines to show the field's height." />
            </Field>
            <Field label="Checkbox">
              <Label htmlFor="ks-checkbox" className="gap-2">
                <Checkbox id="ks-checkbox" defaultChecked />
                Checked
              </Label>
            </Field>
            <Field label="Tag entry (pill)">
              <div className="flex gap-2">
                <PillInput placeholder="+ tag" />
                <div className="force-focus">
                  <PillInput
                    defaultValue="focused"
                    className="force-focus:w-40 force-focus:border-accent-d force-focus:text-fg"
                  />
                </div>
              </div>
            </Field>
          </div>
        </Section>

        <Section title="Empty states">
          <div className="grid grid-cols-2 gap-4">
            <EmptyState>Nothing here yet.</EmptyState>
            <EmptyState>No folders yet. Right-click here to make one.</EmptyState>
          </div>
        </Section>

        <Section title="Mark — decision 29, neutral not accent-tinted">
          <div className="flex flex-wrap items-end gap-8">
            <div className="flex flex-col items-center gap-2">
              <Mark className="size-32" />
              <span className="text-fg-dim">128px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <Mark className="size-16" />
              <span className="text-fg-dim">64px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <Mark className="size-8" />
              <span className="text-fg-dim">32px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <Mark className="size-5" />
              <span className="text-fg-dim">20px</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              <Mark className="size-4" />
              <span className="text-fg-dim">16px — window bar size</span>
            </div>
            <div className="flex flex-col items-center gap-2">
              {/* The exact context it ships in: a 32px dark bar, mark left,
                  wordmark beside it — see components/WindowBar.tsx. */}
              <div className="flex h-8 items-center gap-1.5 rounded-[4px] border border-line bg-panel pl-2 pr-4">
                <Mark className="size-4" />
                <span className="text-[13px] font-semibold text-fg">GGallery</span>
              </div>
              <span className="text-fg-dim">in the window bar</span>
            </div>
          </div>
        </Section>

        <Section title="Accents">
          <div className="grid grid-cols-3 gap-3">
            {ACCENTS.map((accent) => (
              <div
                key={accent.key}
                data-accent={accent.key}
                className="flex flex-col gap-2 rounded-[6px] border border-line bg-panel p-3"
              >
                <span className="text-fg-mid">{accent.label}</span>
                <div className="flex items-center gap-2">
                  <Button variant="accent" size="sm">
                    Accent
                  </Button>
                  <Badge variant="accent">12</Badge>
                  <span className="size-4 rounded-full border border-accent-d bg-accent" />
                </div>
              </div>
            ))}
          </div>
        </Section>
      </div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-3">
      <h2 className="font-mono uppercase tracking-[0.1em] text-fg-dim">{title}</h2>
      {children}
    </section>
  );
}

function Grid({ columns, children }: { columns: number; children: ReactNode }) {
  return (
    <div
      className="grid items-center gap-3"
      style={{ gridTemplateColumns: `repeat(${columns}, max-content)` }}
    >
      {children}
    </div>
  );
}

/** A grid row is just its cells in source order — `Grid`'s CSS grid places
 *  them, so this exists only to keep each row's cells visually grouped in
 *  JSX rather than to render anything itself. */
function Row({ children }: { children: ReactNode }) {
  return <>{children}</>;
}

function Cell({ children }: { children?: ReactNode }) {
  return <div className="flex items-center">{children}</div>;
}

function HeadCell({ children }: { children: ReactNode }) {
  return (
    <div className="flex items-center font-mono text-[12px] uppercase tracking-[0.08em] text-fg-dim">
      {children}
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="flex items-center gap-3">
      <span className="w-40 shrink-0 text-fg-dim">{label}</span>
      {children}
    </div>
  );
}

function EmptyState({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-[6px] border border-line bg-panel p-4 text-fg-dim">
      {children}
    </div>
  );
}

/** The collapsed band's header row, read from `features/folder/FolderBand.tsx`
 *  rather than rendered live — the real component fetches its detail over
 *  IPC, which this dev-only page has nothing to answer. */
function BandRow({
  label,
  hasChevron,
  expanded,
  status,
  counts,
  thisFolderOnly,
  favourite,
}: {
  label: string;
  hasChevron: boolean;
  expanded?: boolean;
  status?: { label: string; colour: string };
  counts: string;
  thisFolderOnly?: boolean;
  favourite?: boolean;
}) {
  return (
    <div className="flex h-11 items-center gap-2.5 px-2.5">
      {hasChevron ? (
        <div className="flex h-8 min-w-0 items-center gap-2 rounded-[4px] px-1.5">
          <ChevronRight
            className={cn(
              "size-[18px] shrink-0 text-fg-dim transition-transform duration-[120ms] ease-out",
              expanded && "rotate-90",
            )}
          />
          <span className="truncate text-[16px] font-semibold text-fg">{label}</span>
        </div>
      ) : (
        <span className="truncate px-1.5 text-[16px] font-semibold text-fg">{label}</span>
      )}

      {status && (
        <span
          className="h-7 shrink-0 rounded-full border bg-raised px-2.5 text-[13px] leading-7"
          style={{ borderColor: status.colour, color: status.colour }}
        >
          {status.label}
        </span>
      )}

      <span className="truncate font-mono tabular-nums text-fg-dim">{counts}</span>

      <span className="ml-auto flex shrink-0 items-center gap-2">
        {thisFolderOnly && (
          <>
            <Label className="gap-2">
              <Checkbox />
              this folder only
            </Label>
            <Separator />
          </>
        )}

        <span className="flex items-center gap-2">
          <LayoutGrid aria-hidden fill="currentColor" className="size-4 shrink-0 text-fg-dim" />
          <Slider aria-label="Tile size" className="w-24" min={0} max={4} value={[1]} />
          <Square aria-hidden fill="currentColor" className="size-4 shrink-0 text-fg-dim" />
        </span>

        {favourite && (
          <>
            <Separator />
            <IconButton aria-label="Pin to the top">
              <Star />
            </IconButton>
          </>
        )}
      </span>
    </div>
  );
}

/** The expanded panel — cover, one chip row for fields and tags together,
 *  and notes as a single growing line. Empty when `fields`/`tags`/`note`
 *  are omitted, which is the default and commonest state. */
function BandExpanded({
  fields = [],
  tags = [],
  note,
}: {
  fields?: [string, string][];
  tags?: string[];
  note?: string;
}) {
  return (
    <div className="flex gap-3 px-3 pb-3 pt-1">
      <div className="flex size-14 shrink-0 items-center justify-center rounded-[5px] border border-line bg-raised text-fg-dim">
        <ImageIcon className="size-5" />
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex flex-wrap items-center gap-1.5">
          {fields.map(([key, value]) => (
            <span
              key={key}
              className="inline-flex h-7 shrink-0 items-stretch overflow-hidden rounded-[4px] border border-line text-[13px]"
            >
              <span className="flex items-center bg-ground px-2 text-fg-dim">{key}</span>
              <span className="flex items-center bg-raised px-2 text-fg-mid">
                {value || <span className="text-fg-dim">—</span>}
              </span>
            </span>
          ))}
          {tags.map((tag) => (
            <Chip key={tag}>{tag}</Chip>
          ))}
          <span className="inline-flex h-7 shrink-0 items-center rounded-[4px] border border-dashed border-line px-2.5 text-[13px] text-fg-dim">
            ＋ add field
          </span>
          <PillInput placeholder="＋ tag" readOnly />
        </div>

        <div className="mt-2.5 h-8 truncate rounded-[4px] px-1.5 leading-8 text-fg-mid">
          {note ?? <span className="text-fg-dim">Notes…</span>}
        </div>
      </div>
    </div>
  );
}
