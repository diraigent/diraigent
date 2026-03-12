-- Promote goals to first-class containers (epics, features, milestones, etc.)

ALTER TABLE diraigent.goal
  ADD COLUMN goal_type TEXT NOT NULL DEFAULT 'epic'
    CONSTRAINT goal_type_check CHECK (goal_type IN ('epic','feature','milestone','sprint','initiative')),
  ADD COLUMN priority INT NOT NULL DEFAULT 0,
  ADD COLUMN parent_goal_id UUID REFERENCES diraigent.goal(id) ON DELETE SET NULL,
  ADD COLUMN auto_status BOOLEAN NOT NULL DEFAULT false;

CREATE INDEX idx_goal_type ON diraigent.goal(project_id, goal_type);
CREATE INDEX idx_goal_parent ON diraigent.goal(parent_goal_id) WHERE parent_goal_id IS NOT NULL;
CREATE INDEX idx_goal_priority ON diraigent.goal(project_id, priority DESC);
