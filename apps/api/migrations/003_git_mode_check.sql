-- Add DB-level CHECK constraint for git_mode column on project table.
-- Valid values match VALID_GIT_MODES in validation.rs: monorepo, standalone, none.
ALTER TABLE diraigent.project
    ADD CONSTRAINT project_git_mode_check
        CHECK (git_mode = ANY (ARRAY['monorepo'::text, 'standalone'::text, 'none'::text]));
