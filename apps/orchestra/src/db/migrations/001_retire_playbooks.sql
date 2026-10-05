-- Keep unfinished legacy executions for human review rather than resuming a
-- retired stage or implicitly merging work that may still need verification.
UPDATE task_execution
SET state = 'human_review', assigned_agent_id = NULL, claimed_at = NULL,
    completed_at = NULL, state_entered_at = datetime('now'), last_synced_at = NULL
WHERE state NOT IN ('done', 'cancelled', 'human_review')
  AND (playbook_id IS NOT NULL OR playbook_step > 0
       OR state NOT IN ('backlog', 'ready', 'working'));
ALTER TABLE task_execution DROP COLUMN playbook_id;
ALTER TABLE task_execution DROP COLUMN playbook_step;
PRAGMA user_version = 1;
