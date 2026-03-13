-- Add parent_id column to task table for parent-child task linking.
-- When an agent decomposes a task into subtasks, the subtasks reference
-- the parent task via parent_id, creating an auditable decomposition trail.

ALTER TABLE diraigent.task
    ADD COLUMN parent_id UUID REFERENCES diraigent.task(id) ON DELETE SET NULL;

CREATE INDEX idx_task_parent_id ON diraigent.task(parent_id) WHERE parent_id IS NOT NULL;
