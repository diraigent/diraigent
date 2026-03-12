-- Step template table: reusable step definitions for playbook steps.
-- tenant_id = NULL means global/shared template, visible to all tenants.

CREATE TABLE diraigent.step_template (
    id              uuid            NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    tenant_id       uuid            REFERENCES diraigent.tenant(id) ON DELETE CASCADE,
    name            text            NOT NULL,
    description     text,
    model           text,
    budget          double precision,
    allowed_tools   text,
    context_level   text,
    on_complete     text,
    retriable       boolean,
    max_cycles      integer,
    timeout_minutes integer,
    mcp_servers     jsonb,
    agents          jsonb,
    agent           text,
    settings        jsonb,
    env             jsonb,
    vars            jsonb,
    tags            text[]          NOT NULL DEFAULT ARRAY[]::text[],
    metadata        jsonb           NOT NULL DEFAULT '{}'::jsonb,
    created_by      uuid            NOT NULL REFERENCES diraigent.auth_user(user_id),
    created_at      timestamptz     NOT NULL DEFAULT now(),
    updated_at      timestamptz     NOT NULL DEFAULT now()
);

-- Indexes
CREATE INDEX idx_step_template_tenant ON diraigent.step_template (tenant_id);
CREATE INDEX idx_step_template_tags   ON diraigent.step_template USING gin (tags);
CREATE INDEX idx_step_template_name   ON diraigent.step_template (name);

-- Auto-update trigger
CREATE TRIGGER trg_step_template_updated
    BEFORE UPDATE ON diraigent.step_template
    FOR EACH ROW
    EXECUTE FUNCTION diraigent.update_timestamp();
