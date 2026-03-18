-- Per-tenant rate limits and storage quotas.
-- Defaults are generous for self-hosted; tighten for multi-tenant SaaS.

ALTER TABLE diraigent.tenant
    ADD COLUMN plan text NOT NULL DEFAULT 'free',
    ADD COLUMN rate_limit_per_min integer NOT NULL DEFAULT 3000,
    ADD COLUMN max_tasks integer NOT NULL DEFAULT 10000,
    ADD COLUMN max_projects integer NOT NULL DEFAULT 100,
    ADD COLUMN max_agents integer NOT NULL DEFAULT 50;
