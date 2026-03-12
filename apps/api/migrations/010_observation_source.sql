-- Add source tracking to observations so we know where each observation came from.
-- source: free-text label like "dream", "log_monitor", "worker", "implement", "review", "manual"
-- source_task_id: optional FK to the task that was being worked on when the observation was created

ALTER TABLE diraigent.observation ADD COLUMN source text;
ALTER TABLE diraigent.observation ADD COLUMN source_task_id uuid REFERENCES diraigent.task(id) ON DELETE SET NULL;
