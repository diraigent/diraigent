-- Fix priority index: 1 (Critical) should sort before 5 (Lowest)
DROP INDEX IF EXISTS diraigent.idx_task_priority;
CREATE INDEX idx_task_priority ON diraigent.task USING btree (project_id, priority ASC) WHERE (state = ANY (ARRAY['backlog'::text, 'ready'::text]));