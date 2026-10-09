-- Existing project payloads stay central until explicitly migrated.
CREATE TABLE diraigent.project_content_owner (
    project_id uuid PRIMARY KEY REFERENCES diraigent.project(id) ON DELETE CASCADE,
    agent_id uuid NOT NULL REFERENCES diraigent.agent(id) ON DELETE RESTRICT,
    store_id uuid NOT NULL
);
-- Routing information only; no project payloads or user-supplied metadata.
CREATE TABLE diraigent.project_content_ref (
    kind text NOT NULL CHECK (kind IN ('log', 'diff', 'artifact')),
    object_id uuid NOT NULL,
    project_id uuid NOT NULL REFERENCES diraigent.project_content_owner(project_id) ON DELETE CASCADE,
    PRIMARY KEY (kind, object_id)
);
CREATE INDEX project_content_ref_project ON diraigent.project_content_ref(project_id);

-- Serialize activation against workspace encryption changes. Local content has
-- no encryption adapter yet, so concurrent requests must not bypass API guards.
CREATE FUNCTION diraigent.check_content_owner_encryption() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE mode text;
BEGIN
    SELECT t.encryption_mode INTO mode FROM diraigent.tenant t
    JOIN diraigent.project p ON p.tenant_id=t.id WHERE p.id=NEW.project_id
    FOR UPDATE OF t;
    IF mode <> 'none' THEN
        RAISE EXCEPTION 'External content requires an unencrypted workspace'
            USING ERRCODE='23514', CONSTRAINT='content_owner_encryption';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER content_owner_encryption BEFORE INSERT OR UPDATE
ON diraigent.project_content_owner FOR EACH ROW
EXECUTE FUNCTION diraigent.check_content_owner_encryption();

CREATE FUNCTION diraigent.check_tenant_content_encryption() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.encryption_mode <> 'none' AND EXISTS (
        SELECT 1 FROM diraigent.project_content_owner o
        JOIN diraigent.project p ON p.id=o.project_id WHERE p.tenant_id=NEW.id
    ) THEN
        RAISE EXCEPTION 'External content does not support workspace encryption'
            USING ERRCODE='23514', CONSTRAINT='tenant_content_encryption';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tenant_content_encryption BEFORE UPDATE OF encryption_mode
ON diraigent.tenant FOR EACH ROW
EXECUTE FUNCTION diraigent.check_tenant_content_encryption();
