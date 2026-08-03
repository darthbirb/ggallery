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

import { Star } from "lucide-react";
import type { ReactNode } from "react";

import { Chip } from "../components/Chip";
import { Badge } from "../components/ui/badge";
import { Button, IconButton } from "../components/ui/button";
import { Checkbox } from "../components/ui/checkbox";
import { Input, PillInput, Textarea } from "../components/ui/input";
import { Label } from "../components/ui/label";
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
