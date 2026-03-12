-- Add versioning support to playbooks.
-- version: auto-incremented on each update.
-- parent_id: references the source playbook this was forked from.
-- parent_version: the version of the parent at the time of forking.

ALTER TABLE diraigent.playbook
    ADD COLUMN version integer NOT NULL DEFAULT 1,
    ADD COLUMN parent_id uuid REFERENCES diraigent.playbook(id) ON DELETE SET NULL,
    ADD COLUMN parent_version integer;

-- Index for efficiently finding forks of a given playbook.
CREATE INDEX idx_playbook_parent_id ON diraigent.playbook(parent_id) WHERE parent_id IS NOT NULL;
