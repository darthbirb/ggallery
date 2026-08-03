import { X } from "lucide-react";
import { useEffect, useState } from "react";

import { ConfirmDialog } from "../../components/Dialog";
import { IconButton } from "../../components/ui/button";
import { Input } from "../../components/ui/input";
import { formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { TagSummary } from "../../lib/types";

interface TagsSectionProps {
  /** Folder flags/labels and the (future) search index both reflect tag
   *  text — refetch after a rename or delete. */
  onChanged: () => void;
}

/**
 * Rename or delete a tag across the whole library — the minimum that stops
 * the vocabulary rotting, per docs/DESIGN.md "Item operations". Merge,
 * aliases and usage counts stay M8's full tag-management screen.
 *
 * A section inside `SettingsPanel`'s single dialog, not a dialog of its own.
 */
export function TagsSection({ onChanged }: TagsSectionProps) {
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState<TagSummary | null>(null);

  const refresh = async (query: string) => {
    try {
      setTags(await ipc.listTags(query.trim() || null));
    } catch (err) {
      setError(ipc.errorMessage(err));
    }
  };

  useEffect(() => {
    void refresh(filter);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filter]);

  const rename = (tag: TagSummary, value: string) => {
    const trimmed = value.trim();
    if (!trimmed || trimmed === tag.value) return;
    (async () => {
      try {
        await ipc.renameTag(tag.id, trimmed);
        await refresh(filter);
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const remove = (tag: TagSummary) => {
    (async () => {
      try {
        await ipc.deleteTag(tag.id);
        await refresh(filter);
        onChanged();
      } catch (err) {
        setError(ipc.errorMessage(err));
      }
    })();
  };

  const label = (tag: TagSummary) => (tag.key ? `${tag.key}: ${tag.value}` : tag.value);

  return (
    <>
      <Input
        value={filter}
        aria-label="Filter tags"
        onChange={(event) => setFilter(event.target.value)}
        placeholder="Filter…"
        className="mb-3"
      />

      {error && (
        <p className="mb-3 rounded-[4px] border border-danger/40 bg-raised px-3 py-2 text-danger">
          {error}
        </p>
      )}

      <table className="w-full border-collapse text-left">
        <tbody>
          {tags.map((tag) => (
            <tr key={tag.id} className="border-t border-line-soft/60">
              {tag.key !== null && (
                <td className="py-1 pr-1 font-mono text-fg-dim">{tag.key}:</td>
              )}
              <td className="py-1 pr-2" colSpan={tag.key === null ? 2 : 1}>
                <Input
                  defaultValue={tag.value}
                  key={tag.id + tag.value}
                  aria-label={`Rename ${label(tag)}`}
                  onBlur={(event) => rename(tag, event.target.value)}
                  className="border-transparent bg-transparent hover:border-line"
                />
              </td>
              <td className="py-1 pr-2 font-mono text-fg-dim">
                {formatCount(tag.usageCount)}
              </td>
              <td className="py-1">
                <span className="flex justify-end">
                  <IconButton
                    aria-label={`Delete ${label(tag)}`}
                    variant="danger"
                    onClick={() => setRemoving(tag)}
                  >
                    <X />
                  </IconButton>
                </span>
              </td>
            </tr>
          ))}
          {tags.length === 0 && (
            <tr>
              <td colSpan={4} className="py-1.5 text-fg-dim">
                No tags{filter ? " match" : ""} yet.
              </td>
            </tr>
          )}
        </tbody>
      </table>

      {removing && (
        <ConfirmDialog
          open
          onOpenChange={(open) => !open && setRemoving(null)}
          title={`Delete ${label(removing)}?`}
          body={`It is used ${formatCount(removing.usageCount)} time${
            removing.usageCount === 1 ? "" : "s"
          }, and comes off everything it is on. This one is not undoable.`}
          confirmLabel="Delete tag"
          danger
          onConfirm={() => {
            const tag = removing;
            setRemoving(null);
            remove(tag);
          }}
        />
      )}
    </>
  );
}
