-- Allow tenant deletion to cascade to projects.
-- Previously project.tenant_id had no ON DELETE action, blocking tenant cleanup
-- during account deletion.

ALTER TABLE diraigent.project
    DROP CONSTRAINT project_tenant_id_fkey,
    ADD CONSTRAINT project_tenant_id_fkey
        FOREIGN KEY (tenant_id) REFERENCES diraigent.tenant(id)
        ON DELETE CASCADE;