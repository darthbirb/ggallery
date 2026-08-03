/**
 * Folder identity, as a band above the grid.
 *
 * Collapsed it is one line: title, status chip, counts. Clicking expands it to
 * the cover, archetype fields edited in place, tags and notes. Expanded state
 * is **global and remembered** — never per folder, which would reflow the grid
 * on every navigation and is state nobody would curate (docs/DESIGN.md §2).
 *
 * It must look right with **no archetype at all**, which is the default and
 * commonest state: the app ships with none. An empty expanded band shows the
 * cover, the counts and an *＋ add field* control — not a row of blank labels.
 */

import { ChevronRight, Image as ImageIcon, Plus, Star } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

import { Chip } from "../../components/Chip";
import { ContextMenu, DropdownMenu, MenuItem, MenuLabel } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { Button, IconButton } from "../../components/ui/button";
import { Input, PillInput, Textarea } from "../../components/ui/input";
import { formatCount, formatTimeAgo } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type {
  ArchetypeInfo,
  FolderDetail,
  FolderNode,
  FolderStatusDef,
} from "../../lib/types";
import { cn } from "../../lib/utils";
import { useOperations } from "../menus/operations";
import { FolderMenu } from "../menus/FolderMenu";

export interface FolderBandProps {
  folder: FolderNode;
  statuses: FolderStatusDef[];
  archetypes: ArchetypeInfo[];
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  thumbsDir: string;
  /** Bumped by anything that could have changed this folder. */
  refreshToken: number;
  onOpen: (folder: FolderNode) => void;
}

export function FolderBand({
  folder,
  statuses,
  archetypes,
  expanded,
  onExpandedChange,
  thumbsDir,
  refreshToken,
  onOpen,
}: FolderBandProps) {
  const ops = useOperations();
  const [detail, setDetail] = useState<FolderDetail | null>(null);

  const refresh = useCallback(async () => {
    try {
      setDetail(await ipc.getFolder(folder.id));
    } catch {
      setDetail(null);
    }
  }, [folder.id]);

  useEffect(() => {
    void refresh();
  }, [refresh, refreshToken]);

  const status = statuses.find((candidate) => candidate.key === folder.status);
  const counts = detail ?? {
    directCount: folder.directCount,
    totalCount: folder.totalCount,
    subfolderCount: 0,
  };

  const header = (
    <div className="flex h-11 items-center gap-2.5 px-2.5">
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
        <span className="truncate text-[16px] font-semibold text-fg">
          {folder.title}
        </span>
      </button>

      <StatusControl
        status={status}
        statuses={statuses}
        onPick={(picked) => ops.setFolderStatus(folder.id, picked.key, picked.label)}
      />

      <span className="truncate font-mono tabular-nums text-fg-dim">
        {formatCount(counts.directCount)} here
        {counts.totalCount !== counts.directCount && (
          <> · {formatCount(counts.totalCount)} below</>
        )}
      </span>

      <span className="ml-auto flex shrink-0 items-center gap-1">
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
      </span>
    </div>
  );

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
      <section className="border-b border-line bg-panel">
        {header}

        {expanded && (
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
              <Fields
                detail={detail}
                archetypes={archetypes}
                onSetLabel={async (key, value) => {
                  await ops.setFolderLabel(folder.id, key, value);
                  void refresh();
                }}
                onApplyArchetype={async (archetype) => {
                  await ops.applyArchetype(folder.id, archetype.id, archetype.name);
                  void refresh();
                }}
              />

              <Tags
                detail={detail}
                onAdd={async (value) => {
                  await ops.addFolderFlag(folder.id, value);
                  void refresh();
                }}
                onRemove={async (tagId) => {
                  await ops.removeFolderTag(folder.id, tagId);
                  void refresh();
                }}
              />

              <Notes
                key={`${folder.id}:${detail?.notes ?? ""}`}
                initial={detail?.notes ?? ""}
                onCommit={(notes) => ops.setFolderNotes(folder.id, notes || null)}
              />

              <p className="mt-2.5 flex flex-wrap items-center gap-x-3 font-mono tabular-nums text-fg-dim">
                <span>{formatCount(counts.directCount)} items here</span>
                <span>{formatCount(counts.totalCount)} in total</span>
                {"subfolderCount" in counts && (
                  <span>{formatCount(counts.subfolderCount)} subfolders</span>
                )}
                {detail?.lastAddedAt != null && (
                  <span>last added {formatTimeAgo(detail.lastAddedAt)}</span>
                )}
              </p>
            </div>
          </div>
        )}
      </section>
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
    <div className="flex w-20 shrink-0 flex-col items-center gap-1">
      <div className="size-20 overflow-hidden rounded-[5px] border border-line bg-raised">
        {thumb ? (
          <img
            src={ipc.assetUrl(thumbsDir, thumb)}
            alt=""
            draggable={false}
            className="h-full w-full object-cover"
          />
        ) : (
          <div className="grid h-full w-full place-items-center text-fg-dim">
            <ImageIcon className="size-6" />
          </div>
        )}
      </div>
      {chosen ? (
        <Button size="sm" className="w-full" onClick={onClear}>
          Clear cover
        </Button>
      ) : (
        <p className="text-[13px] text-fg-dim">automatic</p>
      )}
    </div>
  );
}

function Fields({
  detail,
  archetypes,
  onSetLabel,
  onApplyArchetype,
}: {
  detail: FolderDetail | null;
  archetypes: ArchetypeInfo[];
  onSetLabel: (key: string, value: string) => void;
  onApplyArchetype: (archetype: ArchetypeInfo) => void;
}) {
  const [adding, setAdding] = useState(false);
  const fields = detail?.fields ?? [];

  return (
    <div>
      {fields.length > 0 && (
        <dl className="mb-2 grid grid-cols-[minmax(72px,auto)_1fr] items-center gap-x-3 gap-y-1">
          {fields.map((field) => (
            <div key={field.key} className="contents">
              <dt className="truncate text-fg-dim">{field.key}</dt>
              <dd className="min-w-0">
                <EditableText
                  value={field.value}
                  placeholder="—"
                  label={field.key}
                  onCommit={(value) => value !== field.value && onSetLabel(field.key, value)}
                />
              </dd>
            </div>
          ))}
        </dl>
      )}

      <div className="flex flex-wrap items-center gap-2">
        {detail?.archetypeName ? (
          <span className="font-mono uppercase tracking-[0.1em] text-fg-dim">
            {detail.archetypeName}
          </span>
        ) : (
          archetypes.length > 0 && (
            <DropdownMenu
              trigger={<Button size="sm">Apply an archetype…</Button>}
            >
              <MenuLabel>Archetypes</MenuLabel>
              {archetypes.map((archetype) => (
                <MenuItem key={archetype.id} onSelect={() => onApplyArchetype(archetype)}>
                  {archetype.name}
                </MenuItem>
              ))}
            </DropdownMenu>
          )
        )}

        {adding ? (
          <NewFieldInput
            onCancel={() => setAdding(false)}
            onCommit={(key) => {
              setAdding(false);
              // A label with an empty value still exists and still renders —
              // that is what makes a waiting field visible.
              onSetLabel(key, "");
            }}
          />
        ) : (
          <Button size="sm" onClick={() => setAdding(true)}>
            <Plus />
            add field
          </Button>
        )}
      </div>
    </div>
  );
}

function NewFieldInput({
  onCommit,
  onCancel,
}: {
  onCommit: (key: string) => void;
  onCancel: () => void;
}) {
  const [key, setKey] = useState("");
  return (
    <Input
      autoFocus
      value={key}
      placeholder="field name"
      aria-label="New field name"
      onChange={(event) => setKey(event.target.value)}
      onBlur={() => (key.trim() ? onCommit(key.trim()) : onCancel())}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setKey("");
          onCancel();
        }
      }}
      className="w-36 border-accent-d"
    />
  );
}

function Tags({
  detail,
  onAdd,
  onRemove,
}: {
  detail: FolderDetail | null;
  onAdd: (value: string) => void;
  onRemove: (tagId: number) => void;
}) {
  const [value, setValue] = useState("");
  return (
    <div className="mt-2 flex flex-wrap items-center gap-1.5">
      {(detail?.flags ?? []).map((flag) => (
        <Chip
          key={flag.tagId}
          onRemove={() => onRemove(flag.tagId)}
          removeLabel={`Remove ${flag.value}`}
        >
          {flag.value}
        </Chip>
      ))}
      <PillInput
        value={value}
        placeholder="＋ tag"
        aria-label="Add a tag to this folder"
        onChange={(event) => setValue(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && value.trim()) {
            onAdd(value.trim());
            setValue("");
          }
        }}
      />
    </div>
  );
}

function Notes({
  initial,
  onCommit,
}: {
  initial: string;
  onCommit: (notes: string) => void;
}) {
  return (
    <Textarea
      defaultValue={initial}
      placeholder="Notes…"
      aria-label="Folder notes"
      rows={2}
      onBlur={(event) => {
        if (event.target.value !== initial) onCommit(event.target.value);
      }}
      className="mt-2.5 text-fg-mid"
    />
  );
}

function EditableText({
  value,
  placeholder,
  label,
  onCommit,
}: {
  value: string;
  placeholder?: string;
  label: string;
  onCommit: (value: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(value);

  if (!editing) {
    return (
      <button
        type="button"
        onClick={() => {
          setDraft(value);
          setEditing(true);
        }}
        className="h-8 w-full truncate rounded-[4px] border border-transparent px-2 text-left text-fg-mid hover:border-line hover:bg-raised hover:text-fg"
      >
        {value || <span className="text-fg-dim">{placeholder ?? "click to edit"}</span>}
      </button>
    );
  }

  return (
    <Input
      autoFocus
      aria-label={label}
      value={draft}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        setEditing(false);
        onCommit(draft);
      }}
      onKeyDown={(event) => {
        if (event.key === "Enter") event.currentTarget.blur();
        if (event.key === "Escape") {
          setDraft(value);
          setEditing(false);
        }
      }}
      className="border-accent-d"
    />
  );
}
