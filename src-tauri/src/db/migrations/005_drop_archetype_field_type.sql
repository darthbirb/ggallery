-- M2.5a.1: `archetype_field.type` is dead weight.
--
-- It was one of text | handle | url | date | number, and the only behaviour
-- any of them ever implied was `handle` rendering as a link to a platform
-- profile. PLAN.md locked decision 21 removed that ("the app ships with no
-- domain vocabulary"), and nothing has read the column since: the folder band
-- renders every field the same way, and no validation looks at it.
--
-- A typed-field system that does nothing is worse than no typed-field system:
-- the editor asks a question with no consequence, and the next milestone that
-- touches archetypes has to work out whether the answer matters. It does not.
--
-- The UNIQUE constraint is on (archetype_id, key), so the column is not
-- indexed and DROP COLUMN is legal here.

ALTER TABLE archetype_field DROP COLUMN type;
