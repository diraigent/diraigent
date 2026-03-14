-- Event-observation rules: define mappings from events to auto-created observations.
-- When an event matches a rule's criteria (kind, source, severity threshold),
-- the system auto-creates an observation using the rule's templates.

CREATE TABLE diraigent.event_observation_rule (
    id                    UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id            UUID NOT NULL REFERENCES diraigent.project(id) ON DELETE CASCADE,
    name                  TEXT NOT NULL,
    enabled               BOOLEAN NOT NULL DEFAULT true,
    event_kind            TEXT,          -- NULL = match any event kind
    event_source          TEXT,          -- NULL = match any event source
    severity_gte          TEXT,          -- minimum event severity to trigger (NULL = any)
    observation_kind      TEXT NOT NULL DEFAULT 'insight',
    observation_severity  TEXT NOT NULL DEFAULT 'info',
    title_template        TEXT NOT NULL, -- supports {{event.title}} {{event.kind}} {{event.source}}
    description_template  TEXT,          -- optional template for observation description
    created_at            TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now(),
    updated_at            TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT now()
);

CREATE INDEX idx_event_observation_rule_project_id
    ON diraigent.event_observation_rule(project_id);

CREATE INDEX idx_event_observation_rule_matching
    ON diraigent.event_observation_rule(project_id, enabled, event_kind, event_source);
