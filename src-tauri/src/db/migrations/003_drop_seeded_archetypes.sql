-- M2.1: 002_folder_metadata.sql briefly seeded Person/Place/Event archetypes
-- with social-platform fields — a violation of PLAN.md locked decision 21
-- ("the app ships with no domain vocabulary"), caught before any real
-- release. 002 no longer inserts them, so this is a no-op on any library
-- created fresh from here; it only matters for a test library that already
-- ran the old 002.
--
-- `archetype_field` cascades via its existing `ON DELETE CASCADE` FK on
-- `archetype_id`. Folder labels already created from those fields (plain
-- `folder_tag`/`tag` rows, e.g. `instagram: @ana`) are left exactly where
-- they are — they just stop being treated as "this archetype's fields",
-- consistent with "folders carry labels independently" in docs/DESIGN.md.

UPDATE folder SET archetype_id = NULL WHERE archetype_id IN (1, 2, 3);
DELETE FROM archetype WHERE id IN (1, 2, 3);
