-- Rename goal -> work across all tables, columns, indices, constraints, triggers

-- 1. Rename tables
ALTER TABLE diraigent.goal RENAME TO work;
ALTER TABLE diraigent.goal_comment RENAME TO work_comment;
ALTER TABLE diraigent.task_goal RENAME TO task_work;

-- 2. Rename FK columns
ALTER TABLE diraigent.work_comment RENAME COLUMN goal_id TO work_id;
ALTER TABLE diraigent.task_work RENAME COLUMN goal_id TO work_id;

-- 3. Rename parent_goal_id column in work table
ALTER TABLE diraigent.work RENAME COLUMN parent_goal_id TO parent_work_id;

-- 4. Rename goal_type column to work_type
ALTER TABLE diraigent.work RENAME COLUMN goal_type TO work_type;

-- 5. Rename indices
ALTER INDEX diraigent.idx_goal_project RENAME TO idx_work_project;
ALTER INDEX diraigent.idx_goal_status RENAME TO idx_work_status;
ALTER INDEX diraigent.idx_goal_type RENAME TO idx_work_type;
ALTER INDEX diraigent.idx_goal_parent RENAME TO idx_work_parent;
ALTER INDEX diraigent.idx_goal_priority RENAME TO idx_work_priority;
ALTER INDEX diraigent.idx_goal_sort_order RENAME TO idx_work_sort_order;
ALTER INDEX diraigent.idx_goal_orchestration RENAME TO idx_work_orchestration;
ALTER INDEX diraigent.idx_task_goal_goal RENAME TO idx_task_work_work;
ALTER INDEX diraigent.idx_goal_comment_goal RENAME TO idx_work_comment_work;

-- 6. Rename constraints
ALTER TABLE diraigent.work RENAME CONSTRAINT goal_type_check TO work_type_check;
ALTER TABLE diraigent.work RENAME CONSTRAINT goal_intent_type_check TO work_intent_type_check;
ALTER TABLE diraigent.work RENAME CONSTRAINT goal_status_check TO work_status_check;
ALTER TABLE diraigent.work RENAME CONSTRAINT goal_project_id_fkey TO work_project_id_fkey;

-- 7. Rename triggers
ALTER TRIGGER trg_goal_updated ON diraigent.work RENAME TO trg_work_updated;
ALTER TRIGGER trg_goal_comment_updated ON diraigent.work_comment RENAME TO trg_work_comment_updated;
