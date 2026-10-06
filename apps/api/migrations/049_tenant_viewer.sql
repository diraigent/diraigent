-- Authenticated demo users browse the normal app without write authority.
ALTER TABLE diraigent.tenant_member DROP CONSTRAINT tenant_member_role_check;
ALTER TABLE diraigent.tenant_member ADD CONSTRAINT tenant_member_role_check
    CHECK (role IN ('owner', 'admin', 'member', 'viewer'));
