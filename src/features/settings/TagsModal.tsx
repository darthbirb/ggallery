import { useEffect, useState } from "react";

import { formatCount } from "../../lib/format";
import * as ipc from "../../lib/ipc";
import type { TagSummary } from "../../lib/types";

interface TagsModalProps {
  onClose: () => void;
  /** Folder flags/labels and the (future) search index both reflect tag
   *  text — refetch after a rename or delete. */
  onChanged: () => void;
}

/**
 * Rename or delete a tag across the whole library — the minimum that stops
 * the vocabulary rotting, per docs/DESIGN.md "Item operations". Merge,
 * aliases and usage counts stay M8's full tag-management screen.
 */
export function TagsModal({ onClose, onChanged }: TagsModalProps) {
  const [tags, setTags] = useState<TagSummary[]>([]);
  const [filter, setFilter] = useState("");
  const [error, setError] = useState<string | null>(null);

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
    const label = tag.key ? `${tag.key}: ${tag.value}` : tag.value;
    if (!confirm(`Delete "${label}"? It's used ${tag.usageCount} time(s).`)) return;
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

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="flex max-h-[82vh] w-[460px] flex-col overflow-hidden rounded-[6px] border border-line bg-panel shadow-xl">
        <header className="flex items-center gap-2 border-b border-line px-4 py-3">
          <span className="text-[14px] font-semibold">Tags</span>
          <button
            type="button"
            onClick={onClose}
            className="ml-auto rounded-[3px] px-1.5 text-fg-dim hover:bg-hover hover:text-fg"
          >
            ✕
          </button>
        </header>

        <div className="border-b border-line px-4 py-2">
          <input
            value={filter}
            onChange={(event) => setFilter(event.target.value)}
            placeholder="Filter…"
            className="w-full rounded-[3px] border border-line-soft bg-ground px-1.5 py-0.5 text-fg placeholder:text-fg-dim"
          />
        </div>

        <div className="min-h-0 flex-1 overflow-y-auto p-4 text-[13px]">
          {error && (
            <p className="mb-3 rounded-[3px] border border-danger/40 bg-raised px-3 py-2 text-danger">
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
                    <input
                      defaultValue={tag.value}
                      key={tag.id + tag.value}
                      onBlur={(event) => rename(tag, event.target.value)}
                      className="w-full rounded-[3px] border border-transparent bg-transparent px-1 py-0.5 text-fg hover:border-line-soft focus:border-accent-d"
                    />
                  </td>
                  <td className="py-1 pr-2 font-mono text-[11px] text-fg-dim">
                    {formatCount(tag.usageCount)}
                  </td>
                  <td className="py-1 text-right">
                    <button
                      type="button"
                      onClick={() => remove(tag)}
                      className="px-1 text-fg-dim hover:text-danger"
                    >
                      ×
                    </button>
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
        </div>
      </div>
    </div>
  );
}
