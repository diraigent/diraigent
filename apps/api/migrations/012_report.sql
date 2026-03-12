-- Report table: stores user-requested analysis reports.
CREATE TABLE diraigent.report (
    id          uuid        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    project_id  uuid        NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    title       text        NOT NULL,
    kind        text        NOT NULL,
    prompt      text        NOT NULL,
    status      text        NOT NULL DEFAULT 'pending',
    result      text,
    task_id     uuid        REFERENCES diraigent.task(id) ON DELETE SET NULL,
    created_by  uuid        NOT NULL,
    metadata    jsonb       NOT NULL DEFAULT '{}',
    created_at  timestamptz NOT NULL DEFAULT now(),
    updated_at  timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX idx_report_project_id ON diraigent.report (project_id);
CREATE INDEX idx_report_status ON diraigent.report (status);

-- Auto-update updated_at trigger
CREATE TRIGGER set_report_updated_at
    BEFORE UPDATE ON diraigent.report
    FOR EACH ROW
    EXECUTE FUNCTION diraigent.update_timestamp();
