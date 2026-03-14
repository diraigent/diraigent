-- Remove target_date column from goal table
ALTER TABLE diraigent.goal DROP COLUMN IF EXISTS target_date;
