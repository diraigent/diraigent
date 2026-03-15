-- Add state_entered_at to track when a task entered its current state.
-- Used for staleness scoring.
ALTER TABLE diraigent.task
    ADD COLUMN state_entered_at TIMESTAMPTZ NOT NULL DEFAULT now();

-- Backfill existing rows: use updated_at as a reasonable proxy.
UPDATE diraigent.task SET state_entered_at = updated_at;
