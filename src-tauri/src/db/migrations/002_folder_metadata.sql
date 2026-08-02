-- M2: seed data only. Every table this touches already exists — see
-- 001_initial.sql. Migrations are never edited once shipped.

INSERT INTO folder_status (key, label, colour, ordinal) VALUES
  ('active',   'Active',   '#6b7280', 0),
  ('wip',      'WIP',      '#eab308', 1),
  ('done',     'Done',     '#22c55e', 2),
  ('archived', 'Archived', '#64748b', 3);

-- Archetypes ship empty — PLAN.md locked decision 21, "the app ships with no
-- domain vocabulary". A migration that seeded Person/Place/Event with
-- social-platform fields shipped here briefly and was removed in M2.1's
-- 003_drop_seeded_archetypes.sql; the archetype editor (also M2.1) is what
-- makes an empty seed viable.
