-- Drop the unused target_date column from work (formerly goal) table.
ALTER TABLE diraigent.work DROP COLUMN IF EXISTS target_date;
