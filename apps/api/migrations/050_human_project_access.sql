CREATE TABLE diraigent.project_user_access (
    project_id uuid NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    user_id uuid NOT NULL REFERENCES diraigent.auth_user(user_id) ON DELETE CASCADE,
    role text NOT NULL CHECK (role IN ('viewer', 'editor', 'manager')),
    PRIMARY KEY (project_id, user_id)
);
-- Preserve existing members' access to existing projects. Future access is explicit.
INSERT INTO diraigent.project_user_access (project_id, user_id, role)
SELECT p.id, tm.user_id, 'manager' FROM diraigent.project p
JOIN diraigent.tenant_member tm ON tm.tenant_id=p.tenant_id
WHERE tm.role='member';

-- Leaving a workspace revokes explicit grants, including on a later rejoin.
CREATE FUNCTION diraigent.revoke_departed_user_project_access() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
    DELETE FROM diraigent.project_user_access a USING diraigent.project p
    WHERE a.project_id=p.id AND p.tenant_id=OLD.tenant_id AND a.user_id=OLD.user_id;
    RETURN OLD;
END;
$$;
CREATE TRIGGER revoke_departed_user_project_access
AFTER DELETE ON diraigent.tenant_member FOR EACH ROW
EXECUTE FUNCTION diraigent.revoke_departed_user_project_access();

CREATE FUNCTION diraigent.human_project_role(uid uuid, pid uuid) RETURNS text
LANGUAGE sql STABLE AS $$
    SELECT CASE
        WHEN tm.role IN ('owner','admin') THEN 'manager'
        WHEN p.owner_id=uid THEN CASE WHEN tm.role='viewer' THEN 'viewer' ELSE 'manager' END
        WHEN tm.role='viewer' AND a.role IS NOT NULL THEN 'viewer'
        ELSE a.role END
    FROM diraigent.project p
    JOIN diraigent.tenant_member tm ON tm.tenant_id=p.tenant_id AND tm.user_id=uid
    LEFT JOIN diraigent.project_user_access a ON a.project_id=p.id AND a.user_id=uid
    WHERE p.id=pid
$$;
