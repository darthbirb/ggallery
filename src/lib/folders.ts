import type { FolderNode } from "./types";

/** A folder and every ancestor above it, root-first — by title, since there
 *  is nothing else left to walk by (decision 30 dropped `rel_path`
 *  entirely). There is no library-root row to stop short of any more: a
 *  top-level folder's own `parentId` is `null`, and the walk includes that
 *  folder itself as the last entry, which is what lets one function serve
 *  both an item's ancestry and a folder's own breadcrumb. */
export function ancestorTitles(folders: FolderNode[], folderId: number): string[] {
  const byId = new Map(folders.map((node) => [node.id, node]));
  const titles: string[] = [];
  let current: FolderNode | undefined = byId.get(folderId);
  while (current) {
    titles.unshift(current.title);
    current = current.parentId !== null ? byId.get(current.parentId) : undefined;
  }
  return titles;
}
