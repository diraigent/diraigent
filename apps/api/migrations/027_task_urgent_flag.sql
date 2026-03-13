-- Replace integer priority with boolean urgent flag.
-- All tasks currently use priority=2 (or similar) and numeric priority is not meaningful.
-- Urgent tasks sort first; within urgent/non-urgent, FIFO by created_at.

ALTER TABLE diraigent.task ADD COLUMN urgent BOOLEAN NOT NULL DEFAULT FALSE;

-- Drop the old priority column and its index (if any).
DROP INDEX IF EXISTS diraigent.idx_task_priority;
ALTER TABLE diraigent.task DROP COLUMN priority;
