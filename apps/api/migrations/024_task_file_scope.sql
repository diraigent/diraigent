-- Add file_scope column to tasks for branch overlap detection.
-- Stores the list of file paths a task intends to modify, enabling
-- the orchestra to detect and serialize overlapping work.
ALTER TABLE diraigent.task ADD COLUMN file_scope text[] NOT NULL DEFAULT '{}'::text[];
