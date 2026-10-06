CREATE TABLE diraigent.mcp_server (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    configuration jsonb NOT NULL,
    revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
    enabled boolean NOT NULL DEFAULT false,
    approved_revision bigint,
    approved_by uuid REFERENCES diraigent.auth_user(user_id) ON DELETE SET NULL,
    approved_at timestamptz,
    credential_keys text[] NOT NULL DEFAULT '{}',
    CHECK (NOT enabled OR (approved_revision IS NOT NULL AND approved_revision = revision AND approved_by IS NOT NULL)),
    CHECK ((approved_revision IS NULL) = (approved_by IS NULL))
);
CREATE INDEX mcp_server_project_idx ON diraigent.mcp_server(project_id, id);

-- Account deletion must revoke approval rather than block account deletion or
-- leave an enabled server whose approver no longer exists.
CREATE FUNCTION diraigent.revoke_mcp_approval_without_actor() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.approved_by IS NULL THEN
        NEW.enabled := false;
        NEW.approved_revision := NULL;
        NEW.approved_at := NULL;
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER mcp_server_approval_actor
    BEFORE UPDATE OF approved_by ON diraigent.mcp_server
    FOR EACH ROW EXECUTE FUNCTION diraigent.revoke_mcp_approval_without_actor();

-- No credential values are stored on public registry rows.
CREATE TABLE diraigent.mcp_server_credentials (
    server_id uuid PRIMARY KEY REFERENCES diraigent.mcp_server(id) ON DELETE CASCADE,
    secret jsonb NOT NULL
);
