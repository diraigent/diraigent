-- Add theme preference columns to tenant table for cross-device syncing
ALTER TABLE diraigent.tenant
  ADD COLUMN IF NOT EXISTS theme_preference text NOT NULL DEFAULT 'system',
  ADD COLUMN IF NOT EXISTS accent_color text NOT NULL DEFAULT 'blue';
