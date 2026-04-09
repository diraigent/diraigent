-- Replace UUID FK playbook references with string name references.
-- YAML files in .diraigent/playbooks/ become the single source of truth.

ALTER TABLE diraigent.task ADD COLUMN playbook_name text;
ALTER TABLE diraigent.project ADD COLUMN default_playbook_name text;

-- Backfill names from existing playbook records
UPDATE diraigent.task t
    SET playbook_name = p.title
    FROM diraigent.playbook p
    WHERE t.playbook_id = p.id;

UPDATE diraigent.project proj
    SET default_playbook_name = p.title
    FROM diraigent.playbook p
    WHERE proj.default_playbook_id = p.id;

-- Drop old FK columns (constraints drop automatically with columns in Postgres)
ALTER TABLE diraigent.task DROP COLUMN IF EXISTS playbook_id;
ALTER TABLE diraigent.project DROP COLUMN IF EXISTS default_playbook_id;

-- Drop the playbook table (cascades FK constraints from other tables)
DROP TABLE IF EXISTS diraigent.playbook CASCADE;
