-- The library root (`parent_id IS NULL`) is not a folder in the interface —
-- DESIGN.md's "Navigation roots" is explicit that presenting it as a node
-- everything nests under is wrong. But `folders::upsert` auto-tagged every
-- folder's title on creation with no exception for it, so the root's own
-- title (the library directory's name on disk) ended up inherited onto
-- every single item in the library as an unexplained, unremovable flag.
--
-- Clears both the cached effective-tag rows it produced and the root's own
-- `folder_tag` row, so it stops being walked by future rebuilds too. The
-- accompanying Rust fix (`folders::upsert`) stops a fresh root from ever
-- getting one again.

DELETE FROM item_effective_tag
 WHERE origin_id IN (SELECT id FROM folder WHERE parent_id IS NULL)
   AND tag_id IN (
     SELECT tag_id FROM folder_tag
      WHERE source = 'title'
        AND folder_id IN (SELECT id FROM folder WHERE parent_id IS NULL)
   );

DELETE FROM folder_tag
 WHERE source = 'title'
   AND folder_id IN (SELECT id FROM folder WHERE parent_id IS NULL);
