-- Retire configurable task stages. Never auto-complete or merge legacy work.
-- Hold every unfinished task associated with a playbook for explicit review,
-- including previously queued steps whose YAML Git/review policy is unavailable.
UPDATE diraigent.task
SET state = 'human_review',
    assigned_agent_id = NULL,
    claimed_at = NULL,
    completed_at = NULL,
    state_entered_at = now(),
    updated_at = now()
WHERE state NOT IN ('done', 'cancelled', 'human_review')
  AND (playbook_name IS NOT NULL
       OR state NOT IN ('backlog', 'ready', 'working'));

ALTER TABLE diraigent.task DROP COLUMN playbook_name;
ALTER TABLE diraigent.task DROP COLUMN playbook_step;
ALTER TABLE diraigent.project DROP COLUMN default_playbook_name;
DROP TABLE IF EXISTS diraigent.step_template;

ALTER TABLE diraigent.task ADD CONSTRAINT task_state_check
    CHECK (state IN ('backlog', 'ready', 'working', 'human_review', 'done', 'cancelled'));
