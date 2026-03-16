-- Allow deleting an auth_user without violating agent.owner_id FK.
-- Previously this was RESTRICT (default); now agent rows survive but lose ownership.
ALTER TABLE diraigent.agent
    DROP CONSTRAINT agent_owner_id_fkey,
    ADD CONSTRAINT agent_owner_id_fkey
        FOREIGN KEY (owner_id) REFERENCES diraigent.auth_user(user_id)
        ON DELETE SET NULL;
