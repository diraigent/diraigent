-- Add parent_id to task for parent-child task linking (plan decomposition).
ALTER TABLE diraigent.task ADD COLUMN parent_id uuid REFERENCES diraigent.task(id) ON DELETE SET NULL;

-- Index for efficient child lookups.
CREATE INDEX idx_task_parent_id ON diraigent.task(parent_id) WHERE parent_id IS NOT NULL;
