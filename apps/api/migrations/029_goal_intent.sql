-- Add intent_type column and extend goal status lifecycle

-- 1. Add intent_type column (nullable, no default)
ALTER TABLE diraigent.goal ADD COLUMN intent_type TEXT;

-- 2. Add CHECK constraint for intent_type values
ALTER TABLE diraigent.goal ADD CONSTRAINT goal_intent_type_check
    CHECK (intent_type IS NULL OR intent_type IN ('complex', 'simple', 'hotfix', 'investigation', 'refactor'));

-- 3. Drop existing goal_status_check and recreate with extended values
ALTER TABLE diraigent.goal DROP CONSTRAINT IF EXISTS goal_status_check;
ALTER TABLE diraigent.goal ADD CONSTRAINT goal_status_check
    CHECK (status IN ('active', 'achieved', 'abandoned', 'paused', 'ready', 'processing'));

-- 4. Partial index for efficient orchestra polling of ready/processing goals
CREATE INDEX idx_goal_orchestration ON diraigent.goal(project_id, status)
    WHERE status IN ('ready', 'processing');
