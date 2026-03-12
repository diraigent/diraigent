-- Rename DB columns so naming aligns with actual semantics:
--   old project_root → git_root  (the repo directory where .git lives)
--   old git_root     → project_root (optional subpath within the repo, monorepo only)
--
-- A temporary name is required because we are swapping two column names and
-- PostgreSQL does not allow renaming to an already-existing name in one step.

ALTER TABLE diraigent.project RENAME COLUMN project_root TO git_root_new;
ALTER TABLE diraigent.project RENAME COLUMN git_root     TO project_root;
ALTER TABLE diraigent.project RENAME COLUMN git_root_new TO git_root;
