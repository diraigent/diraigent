-- Forgejo CI integration: schema for storing CI run data from Forgejo Actions.
-- forgejo_integration stores per-project Forgejo instance config and credentials.
-- ci_run, ci_job, ci_step store the hierarchy of workflow run -> job -> step.

-- 1. Forgejo integration (per-project config)
CREATE TABLE diraigent.forgejo_integration (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    base_url        text NOT NULL,
    token           text,                -- encrypted at rest via CryptoDb; optional PAT for API access
    webhook_secret  text,                -- raw webhook secret used as HMAC-SHA256 key
    enabled         boolean DEFAULT true NOT NULL,
    created_at      timestamp with time zone DEFAULT now() NOT NULL,
    updated_at      timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT forgejo_integration_project_key UNIQUE (project_id)
);

CREATE INDEX idx_forgejo_integration_project
    ON diraigent.forgejo_integration USING btree (project_id);

CREATE TRIGGER trg_forgejo_integration_updated
    BEFORE UPDATE ON diraigent.forgejo_integration
    FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();

-- 2. CI runs (workflow/pipeline executions)
CREATE TABLE diraigent.ci_run (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    forgejo_run_id  bigint NOT NULL,
    workflow_name   text NOT NULL,
    status          text NOT NULL,
    branch          text,
    commit_sha      text,
    triggered_by    text,
    started_at      timestamp with time zone,
    finished_at     timestamp with time zone,
    created_at      timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT ci_run_project_forgejo_key UNIQUE (project_id, forgejo_run_id)
);

CREATE INDEX idx_ci_run_project
    ON diraigent.ci_run USING btree (project_id);
CREATE INDEX idx_ci_run_status
    ON diraigent.ci_run USING btree (project_id, status);
CREATE INDEX idx_ci_run_branch
    ON diraigent.ci_run USING btree (project_id, branch);

-- 3. CI jobs (individual jobs within a run)
CREATE TABLE diraigent.ci_job (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    ci_run_id       uuid NOT NULL REFERENCES diraigent.ci_run(id) ON DELETE CASCADE,
    name            text NOT NULL,
    status          text NOT NULL,
    runner          text,
    started_at      timestamp with time zone,
    finished_at     timestamp with time zone
);

CREATE INDEX idx_ci_job_run
    ON diraigent.ci_job USING btree (ci_run_id);

-- 4. CI steps (fine-grained steps per job)
CREATE TABLE diraigent.ci_step (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    ci_job_id       uuid NOT NULL REFERENCES diraigent.ci_job(id) ON DELETE CASCADE,
    name            text NOT NULL,
    status          text NOT NULL,
    exit_code       integer,
    started_at      timestamp with time zone,
    finished_at     timestamp with time zone
);

CREATE INDEX idx_ci_step_job
    ON diraigent.ci_step USING btree (ci_job_id);
