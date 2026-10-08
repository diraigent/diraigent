-- Delegation handlers record the authenticated human identity, including when
-- an agent acts on that user's behalf. Preserve legacy agent attribution by
-- mapping it to the agent's owner before changing the foreign key.
ALTER TABLE diraigent.task DROP CONSTRAINT task_delegated_by_fkey;

UPDATE diraigent.task t
SET delegated_by = a.owner_id
FROM diraigent.agent a
WHERE t.delegated_by = a.id;

ALTER TABLE diraigent.task ADD CONSTRAINT task_delegated_by_fkey
    FOREIGN KEY (delegated_by) REFERENCES diraigent.auth_user(user_id)
    ON DELETE SET NULL;
