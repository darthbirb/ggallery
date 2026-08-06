/**
 * The band above the grid — the only chrome scoped to what the grid is
 * showing (docs/DESIGN.md §2). It renders for every scope, not only a real
 * folder: collapsed it is always one line, and the controls that change the
 * grid — tile size, and "this folder only" when a folder is open — live on
 * its right side, moved here from the window bar per decision 28.
 *
 * Only a real folder gets a chevron, a status chip, and something to expand
 * into: Everything, the Sorting Box and Favourites have no identity to edit,
 * so they render as a plain label and stop there.
 *
 * Expanded state is **global and remembered** — never per folder, which
 * would reflow the grid on every navigation and is state nobody would
 * curate (docs/DESIGN.md §2).
 *
 * #### The expanded band is identity, not a form
 *
 * The first build was a data-entry form: counts printed twice in two
 * phrasings, a permanent "Active" status chip, a reserved notes box as the
 * heaviest element on screen, and ~330px spent on a folder with nothing set.
 * The rules this file now follows:
 *
 * - **Counts appear once**, in the header, in prose — never repeated below.
 * - **Status renders only when it is not `Active`** — absence means nothing
 *   to say, the same rule the tree already follows.
 * - **Fields and tags share one chip row**, add controls at its end.
 * - **Notes are one line that grows on focus**, never a reserved box.
 * - **Applying an archetype** is a once-per-folder setup action in the
 *   folder's context menu, not a standing button competing with content.
 *
 * It must look right with **no archetype at all**, which is the default and
 * commonest state: empty means the cover, the counts and one *＋ add field*
 * control — not a row of blank labels. Target ~140px empty.
 */

import { ChevronRight, Image as ImageIcon, LayoutGrid, Square, Star, X } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";

import { Breadcrumb } from "../../components/Breadcrumb";
import { Chip } from "../../components/Chip";
import { ContextMenu, DropdownMenu, MenuItem, MenuLabel } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { Button, IconButton } from "../../components/ui/button";
import { fieldClassName, Input } from "../../components/ui/input";
import { Separator } from "../../components/ui/separator";
import { Slider } from "../../components/ui/slider";
import { ancestorTitles } from "../../lib/folders";
import { formatCount, formatTimeAgo } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type {
  ArchetypeInfo,
  EffectiveTag,
  FolderDetail,
  FolderNode,
  FolderStatusDef,
} from "../../lib/types";
import { cn } from "../../lib/utils";
import { TILE_SIZES } from "../../state/ui";
import { useOperations } from "../menus/operations";
import { FolderMenu } from "../menus/FolderMenu";

export interface FolderBandProps {
  /** `null` for Everything, the Sorting Box and Favourites — scopes with no
   *  folder identity to expand into. */
  folder: FolderNode | null;
  /** The whole tree, for `folder`'s own ancestry breadcrumb. */
  folders: FolderNode[];
  /** What a non-folder scope prints in the title slot. */
  scopeLabel: string;
  /** The grid's current row count — the count a non-folder scope shows. */
  itemCount: number;
  statuses: FolderStatusDef[];
  archetypes: ArchetypeInfo[];
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  thumbsDir: string;
  /** Bumped by anything that could have changed this folder. */
  refreshToken: number;
  onOpen: (folder: FolderNode) => void;
  tileHeight: number;
  onTileHeightChange: (height: number) => void;
  /** Only meaningful when `folder` is set — the Sorting Box and Everything
   *  have no "this folder only" to toggle. */
  recursive: boolean;
  onRecursiveChange: (recursive: boolean) => void;
}

export function FolderBand({
  folder,
  folders,
  scopeLabel,
  itemCount,
  statuses,
  archetypes,
  expanded,
  onExpandedChange,
  thumbsDir,
  refreshToken,
  onOpen,
  tileHeight,
  onTileHeightChange,
  recursive,
  onRecursiveChange,
}: FolderBandProps) {
  const ops = useOperations();
  const [detail, setDetail] = useState<FolderDetail | null>(null);
  const [inherited, setInherited] = useState<EffectiveTag[]>([]);

  // The same breadcrumb the item details panel renders, the folder itself
  // as the last crumb (docs/DESIGN.md §2). A folder always shows where it
  // sits, including a top-level one, where the answer is "at the top" — the
  // crumb list there is just its own title, one entry.
  const crumbs = useMemo(
    () => (folder ? ancestorTitles(folders, folder.id) : []),
    [folders, folder],
  );

  const refresh = useCallback(async () => {
    if (!folder) {
      setDetail(null);
      setInherited([]);
      return;
    }
    try {
      setDetail(await ipc.getFolder(folder.id));
    } catch {
      setDetail(null);
    }
    try {
      setInherited(await ipc.folderInheritedTags(folder.id));
    } catch {
      setInherited([]);
    }
  }, [folder]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshToken]);

  const status = folder ? statuses.find((candidate) => candidate.key === folder.status) : undefined;
  const counts = detail ?? (folder ? { directCount: folder.directCount, totalCount: folder.totalCount } : null);

  const header = (
    <div className="flex h-11 items-center gap-2.5 px-2.5">
      {folder ? (
        <button
          type="button"
          aria-label={expanded ? "Collapse folder details" : "Expand folder details"}
          aria-expanded={expanded}
          onClick={() => onExpandedChange(!expanded)}
          className="flex h-8 min-w-0 items-center gap-2 rounded-[4px] px-1.5 text-left hover:bg-hover"
        >
          {/* A single icon that rotates rather than swapping — decision 27:
              transform, not a conditional pair, and it costs nothing to
              animate. */}
          <ChevronRight
            className={cn(
              "size-[18px] shrink-0 text-fg-dim transition-transform duration-[120ms] ease-out",
              expanded && "rotate-90",
            )}
          />
          <span className="truncate text-[16px] font-semibold text-fg">{folder.title}</span>
        </button>
      ) : (
        <span className="truncate px-1.5 text-[16px] font-semibold text-fg">{scopeLabel}</span>
      )}

      {/* Absence means nothing to say — the same rule the tree's WIP dot
          follows. A permanent "Active" chip is a legend for the default. */}
      {folder && folder.status !== "active" && (
        <StatusControl
          status={status}
          statuses={statuses}
          onPick={(picked) => ops.setFolderStatus(folder.id, picked.key, picked.label)}
        />
      )}

      {/* Counts appear exactly once, here, in prose — never repeated in the
          expanded panel below. */}
      <span className="truncate font-mono tabular-nums text-fg-dim">
        {counts ? (
          <>
            {formatCount(counts.directCount)} here
            {counts.totalCount !== counts.directCount && (
              <> · {formatCount(counts.totalCount)} below</>
            )}
            {"subfolderCount" in counts && counts.subfolderCount > 0 && (
              <> · {formatCount(counts.subfolderCount)} subfolders</>
            )}
            {detail?.lastAddedAt != null && <> · added {formatTimeAgo(detail.lastAddedAt)}</>}
          </>
        ) : (
          <>{formatCount(itemCount)} items</>
        )}
      </span>

      {/* The controls that change the grid — decision 28. Moved here from
          the window bar, which owns the window, not the grid. */}
      <span className="ml-auto flex shrink-0 items-center gap-2">
        {folder && (
          <>
            <Tooltip
              label={recursive ? "Showing everything below — click to scope to here" : "Scoped to here — click to include everything below"}
              side="bottom"
            >
              <Button
                size="sm"
                aria-pressed={!recursive}
                active={!recursive}
                onClick={() => onRecursiveChange(!recursive)}
              >
                {recursive ? "All items" : "Here only"}
              </Button>
            </Tooltip>
            <Separator />
          </>
        )}

        {/* The classic zoom-slider metaphor: many small tiles at one end,
            one large tile at the other. `fill="currentColor"` turns
            lucide's outline squares solid, so a hollow square never reads
            as an empty slot rather than a tile. */}
        <span className="flex items-center gap-2">
          <LayoutGrid aria-hidden fill="currentColor" className="size-4 shrink-0 text-fg-dim" />
          <Slider
            aria-label="Tile size"
            className="w-24"
            min={0}
            max={TILE_SIZES.length - 1}
            value={[Math.max(TILE_SIZES.indexOf(tileHeight), 0)]}
            onValueChange={([index]) => onTileHeightChange(TILE_SIZES[index])}
          />
          <Square aria-hidden fill="currentColor" className="size-4 shrink-0 text-fg-dim" />
        </span>

        {folder && (
          <>
            <Separator />
            <Tooltip
              label={folder.favorite ? "Unpin from the top" : "Pin to the top"}
              side="bottom"
            >
              <IconButton
                aria-label={folder.favorite ? "Unpin from the top" : "Pin to the top"}
                active={folder.favorite}
                onClick={() =>
                  ops.setFolderFavorite(folder.id, folder.title, !folder.favorite)
                }
              >
                <Star className={folder.favorite ? "fill-current" : undefined} />
              </IconButton>
            </Tooltip>
          </>
        )}
      </span>
    </div>
  );

  const body = (
    <section className="border-b border-line bg-panel">
      {header}

      {folder && expanded && (
        // `reveal-down` per decision 27, "band expansion": the band opens
        // downward, so the reveal fades and settles in the same direction
        // rather than teleporting into place. Collapsing stays instant —
        // this is a mount-in animation, not a two-way transition, since
        // the content unmounts (rather than just hiding) to keep it out of
        // tab order while collapsed.
        <div className="reveal-down flex gap-3 px-3 pb-3 pt-1">
          <Cover
            thumbsDir={thumbsDir}
            thumb={detail?.coverThumb ?? null}
            chosen={detail?.coverItemId != null}
            onClear={() => ops.setFolderCover(folder.id, null)}
          />

          <div className="min-w-0 flex-1">
            {/* Where this folder sits — and, since inherited labels below
                come from these same ancestors, where they come from too.
                Always at least one entry: the folder itself. */}
            {crumbs.length > 0 && (
              <div className="mb-1.5">
                <Breadcrumb titles={crumbs} />
              </div>
            )}

            <ChipRow
              detail={detail}
              inherited={inherited}
              onSetLabel={async (key, value) => {
                await ops.setFolderLabel(folder.id, key, value);
                void refresh();
              }}
              onAddFlag={async (value) => {
                await ops.addFolderFlag(folder.id, value);
                void refresh();
              }}
              onRemoveTag={async (tagId) => {
                await ops.removeFolderTag(folder.id, tagId);
                void refresh();
              }}
            />

            <NotesLine
              key={`${folder.id}:${detail?.notes ?? ""}`}
              initial={detail?.notes ?? ""}
              onCommit={(notes) => ops.setFolderNotes(folder.id, notes || null)}
            />
          </div>
        </div>
      )}
    </section>
  );

  if (!folder) return body;

  return (
    <ContextMenu
      menu={
        <FolderMenu
          folder={folder}
          statuses={statuses}
          archetypes={archetypes}
          onOpen={onOpen}
          onEditDetails={() => onExpandedChange(true)}
        />
      }
    >
      {body}
    </ContextMenu>
  );
}

function StatusControl({
  status,
  statuses,
  onPick,
}: {
  status: FolderStatusDef | undefined;
  statuses: FolderStatusDef[];
  onPick: (status: FolderStatusDef) => void;
}) {
  if (statuses.length === 0) return null;
  return (
    <DropdownMenu
      trigger={
        <button
          type="button"
          aria-label="Folder status"
          className="h-7 shrink-0 rounded-full border bg-raised px-2.5 text-[13px] hover:bg-hover"
          style={{
            borderColor: status?.colour ?? "var(--color-line)",
            color: status?.colour ?? "var(--color-fg-dim)",
          }}
        >
          {status?.label ?? "No status"}
        </button>
      }
    >
      <MenuLabel>Status</MenuLabel>
      {statuses.map((candidate) => (
        <MenuItem key={candidate.key} onSelect={() => onPick(candidate)}>
          {candidate.key === status?.key ? `● ${candidate.label}` : candidate.label}
        </MenuItem>
      ))}
    </DropdownMenu>
  );
}

function Cover({
  thumbsDir,
  thumb,
  chosen,
  onClear,
}: {
  thumbsDir: string;
  thumb: string | null;
  chosen: boolean;
  onClear: () => void;
}) {
  return (
    <div className="relative size-14 shrink-0 overflow-hidden rounded-[5px] border border-line bg-raised">
      {thumb ? (
        <img
          src={ipc.assetUrl(thumbsDir, thumb)}
          alt=""
          draggable={false}
          className="h-full w-full object-cover"
        />
      ) : (
        <div className="grid h-full w-full place-items-center text-fg-dim">
          <ImageIcon className="size-5" />
        </div>
      )}
      {chosen && (
        <Tooltip label="Clear cover" side="bottom">
          <button
            type="button"
            aria-label="Clear cover"
            onClick={onClear}
            className="absolute right-0.5 top-0.5 grid size-4 place-items-center rounded-full bg-ground/85 text-fg-dim hover:text-fg"
          >
            <X className="size-3" />
          </button>
        </Tooltip>
      )}
    </div>
  );
}

/** Fields and tags share one row, add controls at its end — the merge
 *  DESIGN.md §2 asks for, one flowing block instead of two separate ones.
 *  They still read as different kinds of things: a field is structured
 *  key/value data, so `FieldChip` is rectangular and two-toned; a flag is
 *  just a word, so it keeps `Chip`'s round pill. **Adding either one is the
 *  same gesture** — a dashed ＋ button that is replaced in place by an
 *  inline input (or, for a label, two), committed on Enter or on clicking
 *  away, cancelled on Escape, and then the button returns. A label and a
 *  tag differ in shape because their data does; the interaction does not. */
function ChipRow({
  detail,
  inherited,
  onSetLabel,
  onAddFlag,
  onRemoveTag,
}: {
  detail: FolderDetail | null;
  inherited: EffectiveTag[];
  onSetLabel: (key: string, value: string) => void;
  onAddFlag: (value: string) => void;
  onRemoveTag: (tagId: number) => void;
}) {
  const [addingLabel, setAddingLabel] = useState(false);
  const [addingTag, setAddingTag] = useState(false);
  const fields = detail?.fields ?? [];
  const flags = detail?.flags ?? [];
  // Inherited before manual, same rule the item details panel follows —
  // the fixed, structural part reads first, ahead of whatever this folder
  // adds on top of it.
  const inheritedFields = inherited.filter((tag) => tag.key !== null);
  // A folder-name tag never renders as a tag — same rule Details.tsx applies
  // to an item, checked the same structural way (`originIsTitle`), not by
  // comparing text against a breadcrumb.
  const inheritedFlags = inherited.filter((tag) => tag.key === null && !tag.originIsTitle);

  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {inheritedFields.map((tag) => (
        <FieldChip key={`inherited-${tag.tagId}`} fieldKey={tag.key!} value={tag.value} muted />
      ))}

      {fields.map((field) => (
        <FieldChip
          key={field.key}
          fieldKey={field.key}
          value={field.value}
          onCommit={(value) => value !== field.value && onSetLabel(field.key, value)}
        />
      ))}

      {inheritedFlags.map((tag) => (
        <Chip key={`inherited-${tag.tagId}`} muted>
          {tag.value}
        </Chip>
      ))}

      {flags.map((flag) => (
        <Chip
          key={flag.tagId}
          onRemove={() => onRemoveTag(flag.tagId)}
          removeLabel={`Remove ${flag.value}`}
        >
          {flag.value}
        </Chip>
      ))}

      {addingLabel ? (
        <NewLabelInput
          onCancel={() => setAddingLabel(false)}
          onCommit={(key, value) => {
            setAddingLabel(false);
            onSetLabel(key, value);
          }}
        />
      ) : (
        <AddChipButton label="＋ add label" onClick={() => setAddingLabel(true)} />
      )}

      {addingTag ? (
        <NewFlagInput
          onCancel={() => setAddingTag(false)}
          onCommit={(value) => {
            setAddingTag(false);
            onAddFlag(value);
          }}
        />
      ) : (
        <AddChipButton label="＋ add tag" onClick={() => setAddingTag(true)} />
      )}
    </div>
  );
}

/** The collapsed state of either add control — one shape for both, so the
 *  only difference between adding a label and adding a tag is what opens
 *  when it is clicked. */
function AddChipButton({ label, onClick }: { label: string; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="inline-flex h-7 shrink-0 items-center rounded-[4px] border border-dashed border-line px-2.5 text-[13px] text-fg-dim hover:border-fg-dim hover:text-fg"
    >
      {label}
    </button>
  );
}

/** Both halves of a new label, side by side — key then value, so it can be
 *  added in one motion instead of creating an empty field and then hunting
 *  for it in the row to give it a value. Enter in the key field moves to the
 *  value field rather than committing, since a label with no value typed yet
 *  is not necessarily a finished one — Enter *there*, or clicking away from
 *  both fields, commits (a still-empty value is fine: a label with an empty
 *  value still exists and still renders). Escape from either cancels the
 *  whole thing. */
function NewLabelInput({
  onCommit,
  onCancel,
}: {
  onCommit: (key: string, value: string) => void;
  onCancel: () => void;
}) {
  const [key, setKey] = useState("");
  const [value, setValue] = useState("");
  const containerRef = useRef<HTMLSpanElement>(null);
  const valueRef = useRef<HTMLInputElement>(null);

  const cancel = () => {
    setKey("");
    setValue("");
    onCancel();
  };

  return (
    <span
      ref={containerRef}
      onBlur={(event) => {
        if (!containerRef.current?.contains(event.relatedTarget as Node | null)) {
          key.trim() ? onCommit(key.trim(), value.trim()) : onCancel();
        }
      }}
      className="inline-flex h-7 shrink-0 items-stretch overflow-hidden rounded-[4px] border border-accent-d text-[13px]"
    >
      <input
        autoFocus
        value={key}
        placeholder="label"
        aria-label="New label name"
        onChange={(event) => setKey(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault();
            valueRef.current?.focus();
          }
          if (event.key === "Escape") cancel();
        }}
        className="h-full w-20 border-r border-line-soft bg-ground px-2 text-fg placeholder:text-fg-dim focus:outline-none"
      />
      <input
        ref={valueRef}
        value={value}
        placeholder="value"
        aria-label="New label value"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter") event.currentTarget.blur();
          if (event.key === "Escape") cancel();
        }}
        className="h-full w-20 bg-raised px-2 text-fg placeholder:text-fg-dim focus:outline-none"
      />
    </span>
  );
}

/** The tag half of the same pattern — one field, since a flag is only ever
 *  a value. */
function NewFlagInput({
  onCommit,
  onCancel,
}: {
  onCommit: (value: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState("");
  return (
    <input
      autoFocus
      value={value}
      placeholder="tag"
      aria-label="New tag"
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => (value.trim() ? onCommit(value.trim()) : onCancel())}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setValue("");
          onCancel();
        }
      }}
      className="h-7 w-28 rounded-[4px] border border-accent-d bg-raised px-2.5 text-[13px] text-fg placeholder:text-fg-dim focus:outline-none"
    />
  );
}

/** A labelled field: two segments — the key, then the value, editable in
 *  place on click. Rectangular and two-toned rather than a pill, so a
 *  structured key/value field never reads as the same kind of thing as a
 *  plain flag; flags keep the round shape, fields do not.
 *
 *  `muted` renders an inherited field: greyed and read-only, since it comes
 *  from an ancestor folder and that is where it changes — the same rule
 *  `Details.tsx`'s `ItemFieldChip` already applies to an item's inherited
 *  labels. */
function FieldChip({
  fieldKey,
  value,
  muted,
  onCommit,
}: {
  fieldKey: string;
  value: string;
  muted?: boolean;
  onCommit?: (value: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (muted) {
    return (
      <span className="inline-flex h-7 max-w-full shrink-0 items-stretch overflow-hidden rounded-[4px] border border-line-soft text-[13px]">
        <span className="flex shrink-0 items-center border-r border-line-soft bg-ground px-2 text-fg-dim">
          {fieldKey}
        </span>
        <span className="flex min-w-0 items-center truncate bg-ground px-2 text-fg-dim">
          {value || <span className="text-fg-dim">—</span>}
        </span>
      </span>
    );
  }

  if (editing) {
    return (
      <span className="inline-flex h-7 shrink-0 items-stretch overflow-hidden rounded-[4px] border border-accent-d text-[13px]">
        <span className="flex shrink-0 items-center border-r border-line-soft bg-ground px-2 text-fg-dim">
          {fieldKey}
        </span>
        <Input
          autoFocus
          aria-label={fieldKey}
          value={draft}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={() => {
            setEditing(false);
            onCommit?.(draft);
          }}
          onKeyDown={(event) => {
            if (event.key === "Enter") event.currentTarget.blur();
            if (event.key === "Escape") {
              setDraft(value);
              setEditing(false);
            }
          }}
          className="h-full w-20 rounded-none border-none bg-raised px-2 text-fg"
        />
      </span>
    );
  }

  return (
    <button
      type="button"
      onClick={() => {
        setDraft(value);
        setEditing(true);
      }}
      className="inline-flex h-7 max-w-full shrink-0 items-stretch overflow-hidden rounded-[4px] border border-line text-[13px] hover:border-fg-dim"
    >
      <span className="flex shrink-0 items-center border-r border-line-soft bg-ground px-2 text-fg-dim">
        {fieldKey}
      </span>
      <span className="flex min-w-0 items-center truncate bg-raised px-2 text-fg-mid">
        {value || <span className="text-fg-dim">—</span>}
      </span>
    </button>
  );
}

/** One line that grows on focus — never a reserved box. Rest shows the note
 *  (or a placeholder) truncated to a single line; editing swaps it for a
 *  textarea that grows with its content and shrinks back on blur. */
function NotesLine({
  initial,
  onCommit,
}: {
  initial: string;
  onCommit: (notes: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const ref = useRef<HTMLTextAreaElement>(null);

  const autoGrow = () => {
    const el = ref.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${el.scrollHeight}px`;
  };

  if (!editing) {
    return (
      <button
        type="button"
        aria-label="Folder notes"
        onClick={() => setEditing(true)}
        className="mt-2.5 block h-8 w-full truncate rounded-[4px] border border-transparent px-1.5 text-left leading-8 text-fg-mid hover:border-line hover:bg-raised hover:text-fg"
      >
        {initial || <span className="text-fg-dim">Notes…</span>}
      </button>
    );
  }

  return (
    <textarea
      ref={ref}
      autoFocus
      aria-label="Folder notes"
      defaultValue={initial}
      rows={1}
      onFocus={autoGrow}
      onInput={autoGrow}
      onBlur={(event) => {
        setEditing(false);
        if (event.target.value !== initial) onCommit(event.target.value);
      }}
      // The base field look with no size classes of its own — deliberately
      // not `Textarea`, whose `min-h-[56px]` is exactly the reserved box
      // this component exists to not have.
      className={cn(fieldClassName, "mt-2.5 resize-none overflow-hidden px-1.5 py-1")}
    />
  );
}
