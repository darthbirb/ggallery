/**
 * The previewed item's identity and details, in two parts.
 *
 * `DetailsHeader` is the pane's title bar — a chevron, the filename, and
 * dimensions · size — and `DetailsBody` is what the chevron opens, **below
 * it**, pushing the media down. That is the reverse of what M2.5a.1 first
 * shipped: details had their own strip above the filmstrip and grew upward,
 * which gave the pane two headers and a band of chrome sitting between the
 * media and the strip. The pane has one header, and it names what you are
 * looking at.
 *
 * Collapsed shows filename, dimensions and size only. Expanded adds duration,
 * codec, dates, source URL and tags — inherited greyed, manual solid.
 * Expanded state is global and remembered, like the folder band's.
 *
 * This is also where item tags are added and removed, which is the capability
 * M2's disposable panel was carrying. It does not disappear with that panel.
 */

import { ChevronRight } from "lucide-react";
import { useEffect, useState } from "react";

import { Chip } from "../../components/Chip";
import { PillInput } from "../../components/ui/input";
import { formatBytes, formatDuration } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { EffectiveTag, ItemDetail } from "../../lib/types";
import { cn } from "../../lib/utils";
import { useOperations } from "../menus/operations";

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
        "flex h-8 min-w-0 flex-1 items-center gap-2 rounded-[4px] px-1.5 text-left",
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
      <span className="min-w-0 truncate text-fg">{item.origName ?? item.diskName}</span>
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

  return (
    // `reveal-down` per decision 27, "details opening": mounted only while
    // expanded (PreviewMode.tsx), so this is the enter animation; there is no
    // exit animation to match, since collapsing unmounts it immediately.
    <section className="reveal-down max-h-[45%] shrink-0 overflow-y-auto border-b border-line bg-panel px-2.5 pb-2.5 pt-1 text-[13px]">
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        <Row label="On disk" value={item.diskName} mono />
        <Row label="Folder" value={item.folderTitle} />
        {item.durationMs != null && (
          <Row label="Duration" value={formatDuration(item.durationMs)} mono />
        )}
        {item.codec && <Row label="Codec" value={item.codec} mono />}
        <Row
          label="Captured"
          value={
            item.capturedAt
              ? `${new Date(item.capturedAt * 1000).toLocaleString()}${
                  // Where the value came from, so a guess is never mistaken
                  // for metadata — DESIGN.md §1 "Items".
                  item.capturedSrc ? ` (${item.capturedSrc})` : ""
                }`
              : "unknown"
          }
        />
        <Row label="Added" value={new Date(item.addedAt * 1000).toLocaleString()} />
        {item.sourceUrl && <Row label="Source" value={item.sourceUrl} mono />}
      </dl>

      <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
        {tags.map((tag) => (
          <Chip
            key={tag.tagId}
            // Inherited tags are greyed and cannot be removed here — they
            // come from the folder, and that is where they change.
            muted={tag.originId !== null}
            onRemove={
              tag.originId === null
                ? () => {
                    void ops.removeItemTag(item.id, tag.tagId).then(async () => {
                      setTags(await ipc.itemEffectiveTags(item.id).catch(() => tags));
                    });
                  }
                : undefined
            }
            removeLabel={`Remove ${tag.value}`}
          >
            {tag.key ? `${tag.key}: ${tag.value}` : tag.value}
          </Chip>
        ))}

        <PillInput
          value={draft}
          placeholder="＋ tag"
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
