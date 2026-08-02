-- M2.1: folders gain a lifecycle (create/rename/move/delete — see
-- docs/DESIGN.md "Folder operations"). Deleting a folder can't hard-delete
-- the row: items still reference it by `folder_id` (FK, foreign_keys=ON),
-- and those items must stay soft-deleted too so the subtree is
-- journal-reconstructable. This mirrors `item.deleted_at` exactly.
--
-- On trash, `db::folders::trash` also rewrites the folder's (and every
-- descendant's) `rel_path` to `.trashed/<id>` — frees the original path for
-- reuse immediately, with no uniqueness gymnastics, since `id` is always
-- unique. See `db::folders::tree`/`get_detail`/`id_for_rel`, which all
-- filter `deleted_at IS NULL`.

ALTER TABLE folder ADD COLUMN deleted_at INTEGER;
