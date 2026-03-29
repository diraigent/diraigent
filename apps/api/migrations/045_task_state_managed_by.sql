-- Phase 0: Add state_managed_by flag to tasks.
-- During migration, tasks can be managed by either the API or the orchestra.
-- Default 'api' preserves current behavior.
ALTER TABLE diraigent.task
    ADD COLUMN IF NOT EXISTS state_managed_by text NOT NULL DEFAULT 'api';

COMMENT ON COLUMN diraigent.task.state_managed_by IS
    'Who owns the task state machine: api (default) or orchestra';
