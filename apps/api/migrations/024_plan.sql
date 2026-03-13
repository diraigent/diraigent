-- Plan entity: groups tasks with an ordered landing (merge) sequence.
-- A plan coordinates related subtasks so their branches merge in a
-- predictable order, avoiding conflicts.

CREATE TABLE diraigent.plan (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      UUID NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    title           TEXT NOT NULL,
    description     TEXT,
    status          TEXT NOT NULL DEFAULT 'active'
                        CHECK (status IN ('active', 'completed', 'cancelled')),
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_by      UUID NOT NULL,
    created_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at      TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE INDEX idx_plan_project_id ON diraigent.plan(project_id);
CREATE INDEX idx_plan_project_status ON diraigent.plan(project_id, status);

-- Add plan_id to task table so tasks can be grouped into a plan.
-- plan_position defines the landing (merge) order within the plan.
ALTER TABLE diraigent.task
    ADD COLUMN plan_id UUID REFERENCES diraigent.plan(id) ON DELETE SET NULL,
    ADD COLUMN plan_position INT NOT NULL DEFAULT 0;

CREATE INDEX idx_task_plan_id ON diraigent.task(plan_id) WHERE plan_id IS NOT NULL;
