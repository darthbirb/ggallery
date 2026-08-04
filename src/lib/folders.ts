import type { FolderNode } from "./types";

/** A folder and every ancestor above it, root-excluded, root-first — by
 *  title, not `rel_path`: `rel_path` is normalised for the filesystem
 *  (case-folded, slug-safe) and does not carry the casing a breadcrumb, or a
 *  dedupe against a real tag's text, needs to match on.
 *
 *  The library root is a real row (`rel_path === ""`) so that every
 *  top-level folder has a non-null `parentId`, but it is never a folder in
 *  the interface (docs/DESIGN.md §2 "Navigation roots") — the walk stops the
 *  instant it reaches a row whose own `parentId` is `null`, without adding
 *  that row's title. Passing the id of the folder itself returns its own
 *  title as the last entry, which is what lets one function serve both an
 *  item's ancestry and a folder's own. */
export function ancestorTitles(folders: FolderNode[], folderId: number): string[] {
  const byId = new Map(folders.map((node) => [node.id, node]));
  const titles: string[] = [];
  let current = byId.get(folderId);
  while (current && current.parentId !== null) {
    titles.unshift(current.title);
    current = byId.get(current.parentId);
  }
  return titles;
}
