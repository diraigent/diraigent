-- Add reverted_at timestamp to track when a task's changes were reverted.
ALTER TABLE diraigent.task ADD COLUMN reverted_at TIMESTAMPTZ;
