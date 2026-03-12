-- Per-user, per-project scratchpad for notes and todos.
CREATE TABLE diraigent.scratchpad (
    id          uuid        NOT NULL DEFAULT gen_random_uuid() PRIMARY KEY,
    user_id     uuid        NOT NULL,
    project_id  uuid        NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    notes       text        NOT NULL DEFAULT '',
    todos       jsonb       NOT NULL DEFAULT '[]',
    updated_at  timestamptz NOT NULL DEFAULT now(),
    UNIQUE (user_id, project_id)
);
