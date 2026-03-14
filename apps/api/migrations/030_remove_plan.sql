-- Add position to task_goal
ALTER TABLE diraigent.task_goal ADD COLUMN position INT NOT NULL DEFAULT 0;

-- Migrate existing plan_position data
UPDATE diraigent.task_goal tg
SET position = t.plan_position
FROM diraigent.task t
WHERE tg.task_id = t.id AND t.plan_id IS NOT NULL AND t.plan_position > 0;

-- Drop plan columns from task
DROP INDEX IF EXISTS diraigent.idx_task_plan_id;
ALTER TABLE diraigent.task DROP COLUMN plan_id;
ALTER TABLE diraigent.task DROP COLUMN plan_position;

-- Drop plan table
DROP TABLE IF EXISTS diraigent.plan;
