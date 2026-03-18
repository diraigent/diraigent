-- Generalize ci_run to support multiple CI providers (not just Forgejo).
-- Adds a `provider` column and renames `forgejo_run_id` to `external_id`.

-- 1. Add provider column with default 'forgejo' for existing rows
ALTER TABLE diraigent.ci_run ADD COLUMN provider text NOT NULL DEFAULT 'forgejo';

-- 2. Rename column
ALTER TABLE diraigent.ci_run RENAME COLUMN forgejo_run_id TO external_id;

-- 3. Drop old unique constraint
ALTER TABLE diraigent.ci_run DROP CONSTRAINT ci_run_project_forgejo_key;

-- 4. Add new unique constraint scoped to (project_id, provider, external_id)
ALTER TABLE diraigent.ci_run ADD CONSTRAINT ci_run_project_provider_external_key
    UNIQUE (project_id, provider, external_id);
