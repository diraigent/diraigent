-- Task execution logs: stores raw PTY output from Claude worker sessions.
-- Allows remote viewing of agent work without accessing the orchestra machine.
CREATE TABLE diraigent.task_log (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    task_id     uuid NOT NULL REFERENCES diraigent.task(id) ON DELETE CASCADE,
    project_id  uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    agent_id    uuid REFERENCES diraigent.agent(id) ON DELETE SET NULL,
    step_name   text NOT NULL DEFAULT 'implement',
    content     text NOT NULL,
    metadata    jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at  timestamp with time zone DEFAULT now() NOT NULL
);

CREATE INDEX idx_task_log_task ON diraigent.task_log(task_id);
CREATE INDEX idx_task_log_project ON diraigent.task_log(project_id, created_at DESC);
