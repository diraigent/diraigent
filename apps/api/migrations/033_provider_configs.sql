-- Provider configuration table for storing per-provider API credentials and endpoint defaults.
-- Rows with project_id NULL represent tenant-level (global) defaults;
-- rows with project_id set are project-specific overrides.
-- The credential resolution function merges project → global, with project overriding.

CREATE TABLE diraigent.provider_config (
    id          uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id   uuid NOT NULL REFERENCES diraigent.tenant(id) ON DELETE CASCADE,
    project_id  uuid REFERENCES diraigent.project(id) ON DELETE CASCADE,
    provider    text NOT NULL,
    api_key     text,            -- encrypted at rest via CryptoDb
    base_url    text,
    default_model text,
    created_at  timestamp with time zone DEFAULT now() NOT NULL,
    updated_at  timestamp with time zone DEFAULT now() NOT NULL
);

-- One global config per provider per tenant (project_id IS NULL).
CREATE UNIQUE INDEX provider_config_tenant_global_key
    ON diraigent.provider_config (tenant_id, provider)
    WHERE project_id IS NULL;

-- One config per provider per project.
CREATE UNIQUE INDEX provider_config_project_provider_key
    ON diraigent.provider_config (project_id, provider)
    WHERE project_id IS NOT NULL;
