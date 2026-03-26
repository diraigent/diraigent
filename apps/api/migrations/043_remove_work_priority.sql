-- Remove priority column from work items
DROP INDEX IF EXISTS diraigent.idx_work_priority;
ALTER TABLE diraigent.work DROP COLUMN IF EXISTS priority;
