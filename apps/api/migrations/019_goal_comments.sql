-- Goal comments / notes
CREATE TABLE diraigent.goal_comment (
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    goal_id uuid NOT NULL REFERENCES diraigent.goal(id) ON DELETE CASCADE,
    agent_id uuid REFERENCES diraigent.agent(id) ON DELETE SET NULL,
    user_id uuid,
    content text NOT NULL,
    metadata jsonb DEFAULT '{}'::jsonb NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL
);

CREATE INDEX idx_goal_comment_goal ON diraigent.goal_comment(goal_id);

CREATE TRIGGER trg_goal_comment_updated
    BEFORE UPDATE ON diraigent.goal_comment
    FOR EACH ROW EXECUTE FUNCTION diraigent.update_timestamp();
