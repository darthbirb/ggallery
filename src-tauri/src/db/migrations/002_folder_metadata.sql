-- M2: seed data only. Every table this touches already exists — see
-- 001_initial.sql. Migrations are never edited once shipped.

INSERT INTO folder_status (key, label, colour, ordinal) VALUES
  ('active',   'Active',   '#6b7280', 0),
  ('wip',      'WIP',      '#eab308', 1),
  ('done',     'Done',     '#22c55e', 2),
  ('archived', 'Archived', '#64748b', 3);

-- Archetypes, per docs/DESIGN.md "Archetypes". A full in-app editor is
-- Settings territory (deliberately minimal until M9) — M2 seeds the
-- documented examples and lets folders apply one.

INSERT INTO archetype (id, name) VALUES
  (1, 'Person'),
  (2, 'Place'),
  (3, 'Event');

INSERT INTO archetype_field (archetype_id, key, type, ordinal) VALUES
  (1, 'instagram', 'handle', 0),
  (1, 'tiktok',    'handle', 1),
  (1, 'youtube',   'handle', 2),
  (1, 'twitter',   'handle', 3),
  (2, 'city',      'text',   0),
  (2, 'country',   'text',   1),
  (2, 'visited',   'date',   2),
  (3, 'date',       'date',  0),
  (3, 'location',   'text',  1);
