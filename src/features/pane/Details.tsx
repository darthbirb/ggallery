/**
 * The previewed item's identity and details, in two parts.
 *
 * `DetailsHeader` is the pane's title bar — a chevron and dimensions · size
 * — and `DetailsBody` is what the chevron opens, **below it**, pushing the
 * media down. That is the reverse of what M2.5a.1 first shipped: details had
 * their own strip above the filmstrip and grew upward, which gave the pane
 * two headers and a band of chrome sitting between the media and the strip.
 * The pane has one header.
 *
 * The filename does not live in the header — the header now shares its row
 * with the pane's own fold and mode controls, and `DetailsBody`'s "File
 * Name"/"Original Name" rows are where it reads instead, expanded only.
 * Collapsed shows dimensions and size only. Expanded adds duration, codec,
 * dates, the folder hierarchy, and fields and tags — inherited greyed,
 * manual solid. Expanded state is global and remembered, like the folder
 * band's.
 *
 * This is also where item tags are added and removed, which is the capability
 * M2's disposable panel was carrying. It does not disappear with that panel.
 */

import { ChevronRight, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { Breadcrumb } from "../../components/Breadcrumb";
import { Chip } from "../../components/Chip";
import { PillInput } from "../../components/ui/input";
import { formatBytes, formatDateTime, formatDuration } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { EffectiveTag, ItemDetail } from "../../lib/types";
import { cn } from "../../lib/utils";
import { useOperations } from "../menus/operations";

/** Inherited (uneditable here) before manual, each group alphabetical —
 *  never one flat alphabetical list, or a folder's own tags scatter among
 *  whatever this item happens to add on top of them instead of reading as
 *  the fixed, structural part they are. */
function compareInheritedFirst(
  a: EffectiveTag,
  b: EffectiveTag,
  sortKey: (tag: EffectiveTag) => string,
): number {
  const rank = (tag: EffectiveTag) => (tag.originId !== null ? 0 : 1);
  const byOrigin = rank(a) - rank(b);
  if (byOrigin !== 0) return byOrigin;
  return sortKey(a).localeCompare(sortKey(b), undefined, { sensitivity: "base" });
}

export function DetailsHeader({
  item,
  expanded,
  onExpandedChange,
}: {
  item: ItemDetail;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
}) {
  const dimensions =
    item.width && item.height ? `${item.width}×${item.height}` : "unknown size";

  return (
    <button
      type="button"
      aria-expanded={expanded}
      onClick={() => onExpandedChange(!expanded)}
      className={cn(
        "flex h-8 flex-1 items-center gap-2 rounded-[4px] px-1.5 text-left",
        "hover:bg-hover",
      )}
    >
      {/* A single icon that rotates rather than swapping — decision 27:
          transform, not a conditional pair. */}
      <ChevronRight
        className={cn(
          "size-[18px] shrink-0 text-fg-dim transition-transform duration-[120ms] ease-out",
          expanded && "rotate-90",
        )}
      />
      <span className="shrink-0 font-mono tabular-nums text-fg-dim">
        {dimensions} · {formatBytes(item.sizeBytes)}
      </span>
    </button>
  );
}

export function DetailsBody({
  item,
  refreshToken,
}: {
  item: ItemDetail;
  refreshToken: number;
}) {
  const ops = useOperations();
  const [tags, setTags] = useState<EffectiveTag[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    let cancelled = false;
    void ipc
      .itemEffectiveTags(item.id)
      .then((list) => !cancelled && setTags(list))
      .catch(() => !cancelled && setTags([]));
    return () => {
      cancelled = true;
    };
  }, [item.id, refreshToken]);

  const addTag = async () => {
    const text = draft.trim();
    if (!text) return;
    const at = text.indexOf(":");
    const key = at === -1 ? null : text.slice(0, at).trim() || null;
    const value = at === -1 ? text : text.slice(at + 1).trim();
    if (!value) return;
    setDraft("");
    await ops.addItemTag([item.id], key, value);
    setTags(await ipc.itemEffectiveTags(item.id).catch(() => tags));
  };

  const removeTag = (tagId: number) => {
    void ops.removeItemTag(item.id, tagId).then(async () => {
      setTags(await ipc.itemEffectiveTags(item.id).catch(() => tags));
    });
  };

  const fileName = item.diskName;
  const originalName = item.origName ?? item.diskName;

  const crumbs = useMemo(() => item.folderBreadcrumb.map((crumb) => crumb.title), [item.folderBreadcrumb]);

  const fields = tags
    .filter((tag): tag is EffectiveTag & { key: string } => tag.key !== null)
    .sort((a, b) => compareInheritedFirst(a, b, (tag) => tag.key ?? ""));
  // Every folder auto-tags itself with its own title (DATA-MODEL's "tag
  // resolution"), which would otherwise repeat every crumb above as a
  // second, tag-shaped copy of the same folder. Only an *inherited* flag is
  // suppressed — a manual one on this item that happens to share the text is
  // a deliberate choice, not the folder leaking through, so it stays. This is
  // structural (`originIsTitle`, joined on the actual contribution), not a
  // string comparison against the breadcrumb — the same tag id can be one
  // folder's title and another folder's manual flag, and only the title
  // contribution is suppressed.
  const flags = tags
    .filter((tag) => tag.key === null && !tag.originIsTitle)
    .sort((a, b) => compareInheritedFirst(a, b, (tag) => tag.value));

  return (
    // `reveal-down` per decision 27, "details opening": mounted only while
    // expanded (PreviewMode.tsx), so this is the enter animation; there is no
    // exit animation to match, since collapsing unmounts it immediately.
    <section className="reveal-down max-h-[45%] shrink-0 overflow-y-auto border-b border-line bg-panel px-2.5 pb-2.5 pt-1 text-[13px]">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        {/* The header dropped the filename to make room for the pane's fold
            and mode controls — these two rows are where it reads now. Both
            mono: they are the same kind of value (a name, verbatim), and
            showing one plain and one mono was the "fonts are inconsistent"
            complaint that got them renamed in the first place. */}
        <Row label="File Name" value={fileName} mono />
        <Row
          label="Original Name"
          value={originalName === fileName ? "—" : originalName}
          mono
        />
        {item.durationMs != null && (
          <Row label="Duration" value={formatDuration(item.durationMs)} mono />
        )}
        {item.codec && <Row label="Codec" value={item.codec} mono />}
        {/* Where the date came from used to be spelled out in words next to
            it — "(exif)", "(mtime)" — which read as debug output, not
            information. `capturedAt` already resolves to the best date
            available (real metadata, or the file's own creation time as a
            fallback — see `media::probe`), so the value alone is enough. */}
        <Row label="Created" value={item.capturedAt ? formatDateTime(item.capturedAt) : "unknown"} />
        <Row label="Added" value={formatDateTime(item.addedAt)} />
        {item.sourceUrl && <Row label="Source" value={item.sourceUrl} mono />}
      </dl>

      <ItemFolderBreadcrumb titles={crumbs} />

      <div className="mt-2 flex flex-wrap items-center gap-1.5">
        {fields.map((field) => (
          <ItemFieldChip
            key={field.tagId}
            label={field.key}
            value={field.value}
            // Inherited fields are greyed and cannot be removed here — they
            // come from the folder, and that is where they change.
            muted={field.originId !== null}
            onRemove={field.originId === null ? () => removeTag(field.tagId) : undefined}
            removeLabel={`Remove ${field.key}`}
          />
        ))}

        {flags.map((tag) => (
          <Chip
            key={tag.tagId}
            muted={tag.originId !== null}
            onRemove={tag.originId === null ? () => removeTag(tag.tagId) : undefined}
            removeLabel={`Remove ${tag.value}`}
          >
            {tag.value}
          </Chip>
        ))}

        <PillInput
          value={draft}
          placeholder="＋ add tag"
          aria-label="Add a tag to this item"
          onChange={(event) => setDraft(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") void addTag();
          }}
        />
      </div>
    </section>
  );
}

/** The item's ancestor folders, root-first — what the "Folder: X" row used to
 *  say, without also repeating the immediate folder a second time as a
 *  tag-shaped chip below (its title is auto-tagged onto every item inside
 *  it; see the dedupe above `ancestorTitles` feeds). An item with no crumbs
 *  is one sitting loose at the top level — flagged in red as "Unsorted"
 *  rather than left blank, since a quiet gap here used to read as a missing
 *  value rather than a real state worth noticing. `FolderBand` renders the
 *  same `Breadcrumb` for a folder's own ancestry, where there is always at
 *  least the folder itself and this case does not arise. */
function ItemFolderBreadcrumb({ titles }: { titles: string[] }) {
  if (titles.length === 0) {
    return (
      <div className="mt-2 flex items-center font-mono text-[12px]">
        <span className="truncate rounded-[3px] border border-danger/40 bg-danger/10 px-1.5 py-0.5 text-danger">
          Unsorted
        </span>
      </div>
    );
  }

  return (
    <div className="mt-2">
      <Breadcrumb titles={titles} />
    </div>
  );
}

/** A read-only twin of `FolderBand`'s `FieldChip` — the same two-tone
 *  rectangle, so a labelled field reads as the same kind of thing whether it
 *  is looked at from a folder or an item. No in-place editing here: this
 *  adds and removes whole fields, it does not rewrite a value. */
function ItemFieldChip({
  label,
  value,
  muted,
  onRemove,
  removeLabel,
}: {
  label: string;
  value: string;
  muted?: boolean;
  onRemove?: () => void;
  removeLabel?: string;
}) {
  return (
    <span
      className={cn(
        "inline-flex h-7 max-w-full shrink-0 items-stretch overflow-hidden rounded-[4px] border text-[13px]",
        muted ? "border-line-soft" : "border-line",
      )}
    >
      <span className="flex shrink-0 items-center border-r border-line-soft bg-ground px-2 text-fg-dim">
        {label}
      </span>
      <span
        className={cn(
          "flex min-w-0 items-center truncate px-2",
          muted ? "bg-ground text-fg-dim" : "bg-raised text-fg-mid",
        )}
      >
        {value || <span className="text-fg-dim">—</span>}
      </span>
      {onRemove && (
        <button
          type="button"
          aria-label={removeLabel ?? "Remove"}
          onClick={onRemove}
          className={cn(
            "grid w-5 shrink-0 place-items-center text-fg-dim hover:bg-danger/20 hover:text-danger",
            muted ? "bg-ground" : "bg-raised",
          )}
        >
          <X className="size-3.5" />
        </button>
      )}
    </span>
  );
}

function Row({
  label,
  value,
  mono,
}: {
  label: string;
  value: string;
  mono?: boolean;
}) {
  return (
    <div className="contents">
      <dt className="text-fg-dim">{label}</dt>
      <dd className={cn("min-w-0 truncate text-fg-mid", mono && "font-mono")}>
        {value}
      </dd>
    </div>
  );
}
