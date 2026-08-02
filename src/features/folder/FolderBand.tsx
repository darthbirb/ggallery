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

import { useCallback, useEffect, useState } from "react";

import { Button, IconButton } from "../../components/Button";
import { Chip } from "../../components/Chip";
import { ContextMenu, DropdownMenu, MenuItem, MenuLabel } from "../../components/Menu";
import { Tooltip } from "../../components/Tooltip";
import { formatCount, formatTimeAgo } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type {
  ArchetypeInfo,
  FolderDetail,
  FolderNode,
  FolderStatusDef,
} from "../../lib/types";
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
    <div className="flex items-center gap-2 px-3 py-1.5">
      <button
        type="button"
        aria-label={expanded ? "Collapse folder details" : "Expand folder details"}
        aria-expanded={expanded}
        onClick={() => onExpandedChange(!expanded)}
        className="flex min-w-0 items-center gap-2 text-left"
      >
        <span className="w-3 shrink-0 text-center text-[9px] text-fg-dim">
          {expanded ? "▾" : "▸"}
        </span>
        <span className="truncate text-[14px] font-semibold text-fg">
          {folder.title}
        </span>
      </button>

      <StatusControl
        status={status}
        statuses={statuses}
        onPick={(picked) => ops.setFolderStatus(folder.id, picked.key, picked.label)}
      />

      <span className="truncate font-mono text-[11px] tabular-nums text-fg-dim">
        {formatCount(counts.directCount)} here
        {counts.totalCount !== counts.directCount && (
          <> · {formatCount(counts.totalCount)} below</>
        )}
      </span>

      <span className="ml-auto flex shrink-0 items-center gap-1">
        <Tooltip label={folder.favorite ? "Unpin from the top" : "Pin to the top"} side="bottom">
          <IconButton
            aria-label={folder.favorite ? "Unpin from the top" : "Pin to the top"}
            active={folder.favorite}
            onClick={() =>
              ops.setFolderFavorite(folder.id, folder.title, !folder.favorite)
            }
          >
            {folder.favorite ? "★" : "☆"}
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
          <div className="flex gap-3 px-3 pb-3 pt-0.5">
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

              <p className="mt-2 flex flex-wrap items-center gap-x-3 font-mono text-[11px] tabular-nums text-fg-dim">
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
          className="shrink-0 rounded-full border px-2 py-[1px] text-[11px] hover:bg-hover"
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
    <div className="shrink-0">
      <div className="h-[76px] w-[76px] overflow-hidden rounded-[4px] border border-line-soft bg-raised">
        {thumb ? (
          <img
            src={ipc.assetUrl(thumbsDir, thumb)}
            alt=""
            draggable={false}
            className="h-full w-full object-cover"
          />
        ) : (
          <div className="grid h-full w-full place-items-center text-fg-dim">▣</div>
        )}
      </div>
      {chosen ? (
        <button
          type="button"
          onClick={onClear}
          className="mt-1 w-[76px] text-center text-[11px] text-fg-dim hover:text-fg"
        >
          clear cover
        </button>
      ) : (
        <p className="mt-1 w-[76px] text-center text-[11px] text-fg-dim">automatic</p>
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
        <dl className="mb-1.5 grid grid-cols-[minmax(64px,auto)_1fr] items-baseline gap-x-3 gap-y-0.5">
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
          <span className="font-mono text-[10px] uppercase tracking-[0.12em] text-fg-dim">
            {detail.archetypeName}
          </span>
        ) : (
          archetypes.length > 0 && (
            <DropdownMenu
              trigger={
                <Button variant="outline">
                  Apply an archetype…
                </Button>
              }
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
          <Button variant="outline" onClick={() => setAdding(true)}>
            ＋ add field
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
    <input
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
      className="w-28 rounded-[4px] border border-accent-d bg-ground px-1.5 py-[2px] text-[12px] text-fg"
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
      <input
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
        className="w-20 rounded-full border border-dashed border-line bg-transparent px-2 py-[1px] text-[12px] text-fg-mid placeholder:text-fg-dim focus:w-32 focus:border-accent-d"
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
    <textarea
      defaultValue={initial}
      placeholder="Notes…"
      aria-label="Folder notes"
      rows={2}
      onBlur={(event) => {
        if (event.target.value !== initial) onCommit(event.target.value);
      }}
      className="mt-2 w-full resize-none rounded-[4px] border border-line-soft bg-ground px-2 py-1 text-[13px] text-fg-mid placeholder:text-fg-dim focus:border-accent-d"
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
        className="w-full truncate text-left text-fg-mid hover:text-fg"
      >
        {value || <span className="text-fg-dim">{placeholder ?? "click to edit"}</span>}
      </button>
    );
  }

  return (
    <input
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
      className="w-full rounded-[3px] border border-accent-d bg-ground px-1 text-fg"
    />
  );
}
