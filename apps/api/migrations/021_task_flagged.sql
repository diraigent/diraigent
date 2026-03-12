-- Add a user-toggleable flagged boolean to tasks.
ALTER TABLE diraigent.task ADD COLUMN flagged BOOLEAN NOT NULL DEFAULT false;
