-- Fix membership unique constraint to be tenant-scoped.
-- The old constraint UNIQUE(agent_id, role_id) prevents the same agent from
-- being assigned to the same role in different tenants.  The correct constraint
-- is UNIQUE(tenant_id, agent_id, role_id) so each tenant has independent
-- membership records.

ALTER TABLE diraigent.membership
  DROP CONSTRAINT membership_agent_id_role_id_key;

ALTER TABLE diraigent.membership
  ADD CONSTRAINT membership_tenant_agent_role_key UNIQUE (tenant_id, agent_id, role_id);
