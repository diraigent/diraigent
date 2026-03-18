-- GitHub CI integration: stores per-project GitHub instance config and credentials.
-- Parallel to forgejo_integration; uses the same ci_run/ci_job/ci_step tables
-- with provider='github'.

CREATE TABLE diraigent.github_integration (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id      uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    base_url        text NOT NULL DEFAULT 'https://api.github.com',
    token           text,                -- encrypted at rest via CryptoDb; optional PAT for API access
    webhook_secret  text,                -- HMAC-SHA256 key used to validate X-Hub-Signature-256
    enabled         boolean DEFAULT true NOT NULL,
    created_at      timestamp with time zone DEFAULT now() NOT NULL,
    updated_at      timestamp with time zone DEFAULT now() NOT NULL,
    CONSTRAINT github_integration_project_key UNIQUE (project_id)
);

CREATE INDEX idx_github_integration_project
    ON diraigent.github_integration USING btree (project_id);

CREATE TRIGGER trg_github_integration_updated
    BEFORE UPDATE ON diraigent.github_integration
    FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
