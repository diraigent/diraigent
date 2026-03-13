-- Add sort_order column to goal table for drag-and-drop ordering
ALTER TABLE diraigent.goal ADD COLUMN sort_order integer NOT NULL DEFAULT 0;

-- Index for efficient ordering queries
CREATE INDEX idx_goal_sort_order ON diraigent.goal(project_id, sort_order);

-- Backfill existing goals with sequential sort_order per project
UPDATE diraigent.goal SET sort_order = sub.rn
FROM (
    SELECT id, ROW_NUMBER() OVER (PARTITION BY project_id ORDER BY priority DESC, created_at DESC) AS rn
    FROM diraigent.goal
) sub
WHERE diraigent.goal.id = sub.id;
