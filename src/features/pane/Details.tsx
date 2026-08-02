/**
 * The preview's details block: small, and collapsible.
 *
 * Collapsed shows filename, dimensions and size only. Expanded adds duration,
 * codec, dates, source URL and tags — inherited greyed, manual solid
 * (docs/DESIGN.md §2). Expanded state is global and remembered, like the
 * folder band's.
 *
 * This is also where item tags are added and removed, which is the capability
 * M2's disposable panel was carrying. It does not disappear with that panel.
 */

import { useEffect, useState } from "react";

import { Chip } from "../../components/Chip";
import { formatBytes, formatDuration } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { EffectiveTag, ItemDetail } from "../../lib/types";
import { useOperations } from "../menus/operations";

export function Details({
  item,
  expanded,
  onExpandedChange,
  refreshToken,
}: {
  item: ItemDetail;
  expanded: boolean;
  onExpandedChange: (expanded: boolean) => void;
  refreshToken: number;
}) {
  const ops = useOperations();
  const [tags, setTags] = useState<EffectiveTag[]>([]);
  const [draft, setDraft] = useState("");

  useEffect(() => {
    if (!expanded) return;
    let cancelled = false;
    void ipc
      .itemEffectiveTags(item.id)
      .then((list) => !cancelled && setTags(list))
      .catch(() => !cancelled && setTags([]));
    return () => {
      cancelled = true;
    };
  }, [item.id, expanded, refreshToken]);

  const dimensions =
    item.width && item.height ? `${item.width}×${item.height}` : "unknown size";

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
    <section className="shrink-0 border-t border-line bg-panel text-[12px]">
      <button
        type="button"
        aria-expanded={expanded}
        onClick={() => onExpandedChange(!expanded)}
        className="flex w-full items-center gap-2 px-2 py-1.5 text-left"
      >
        <span className="w-3 shrink-0 text-center text-[9px] text-fg-dim">
          {expanded ? "▾" : "▸"}
        </span>
        <span className="min-w-0 flex-1 truncate text-fg">
          {item.origName ?? item.diskName}
        </span>
        <span className="shrink-0 font-mono tabular-nums text-fg-dim">
          {dimensions} · {formatBytes(item.sizeBytes)}
        </span>
      </button>

      {expanded && (
        <div className="px-2 pb-2">
          <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
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
                      // Where the value came from, so a guess is never
                      // mistaken for metadata — DESIGN.md §1 "Items".
                      item.capturedSrc ? ` (${item.capturedSrc})` : ""
                    }`
                  : "unknown"
              }
            />
            <Row label="Added" value={new Date(item.addedAt * 1000).toLocaleString()} />
            {item.sourceUrl && <Row label="Source" value={item.sourceUrl} mono />}
          </dl>

          <div className="mt-2 flex flex-wrap items-center gap-1.5">
            {tags.map((tag) => (
              <Chip
                key={tag.tagId}
                // Inherited tags are greyed and cannot be removed here —
                // they come from the folder, and that is where they change.
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

            <input
              value={draft}
              placeholder="＋ tag"
              aria-label="Add a tag to this item"
              onChange={(event) => setDraft(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") void addTag();
              }}
              className="w-20 rounded-full border border-dashed border-line bg-transparent px-2 py-[1px] text-[12px] text-fg-mid placeholder:text-fg-dim focus:w-32 focus:border-accent-d"
            />
          </div>
        </div>
      )}
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
      <dd className={`min-w-0 truncate text-fg-mid ${mono ? "font-mono text-[11px]" : ""}`}>
        {value}
      </dd>
    </div>
  );
}
