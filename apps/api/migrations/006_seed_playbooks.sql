-- Create a "system" user for Diraigent-seeded data.
INSERT INTO diraigent.auth_user (user_id, auth_user_id)
VALUES ('00000000-0000-0000-0000-000000000000', 'system')
ON CONFLICT (auth_user_id) DO NOTHING;

-- Seed default global playbooks (tenant_id = NULL → visible to all tenants).
INSERT INTO diraigent.playbook (tenant_id, title, trigger_description, steps, tags, metadata, initial_state, created_by)
VALUES
(
    NULL,
    'Standard Lifecycle',
    'implement → review → merge',
    '[
        {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
        {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"},
        {"name":"merge","description":"Merge the working branch into main after a passing review.","budget":2.5,"allowed_tools":"merge"}
    ]'::jsonb,
    ARRAY['default'],
    '{}'::jsonb,
    'ready',
    '00000000-0000-0000-0000-000000000000'
),
(
    NULL,
    'Standard (Backlog Start)',
    'backlog → ready → implement → review → merge',
    '[
        {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
        {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"},
        {"name":"merge","description":"Merge the working branch into main after a passing review.","budget":2.5,"allowed_tools":"merge"}
    ]'::jsonb,
    ARRAY['default', 'backlog'],
    '{"start_in_backlog":true}'::jsonb,
    'backlog',
    '00000000-0000-0000-0000-000000000000'
),
(
    NULL,
    'Researcher',
    'scope → gather → synthesize → document',
    '[
        {"name":"scope","description":"Read existing docs and codebase to define the research question. Post the refined scope as a decision.","model":"claude-sonnet-4-6","budget":5.0,"allowed_tools":"readonly"},
        {"name":"gather","description":"Fetch and aggregate relevant sources — code, docs, web references. Post key findings as knowledge entries.","model":"claude-sonnet-4-6","budget":10.0,"allowed_tools":"full"},
        {"name":"synthesize","description":"Synthesize findings into structured notes. Identify patterns, trade-offs, and recommendations.","model":"claude-opus-4-6","budget":15.0,"allowed_tools":"full"},
        {"name":"document","description":"Write the final documentation artifact. Post it as a task artifact.","budget":8.0,"allowed_tools":"full"}
    ]'::jsonb,
    ARRAY['research', 'documentation'],
    '{}'::jsonb,
    'ready',
    '00000000-0000-0000-0000-000000000000'
),
(
    NULL,
    'Dreamer',
    'implement → review → merge → dream',
    '[
        {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
        {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"},
        {"name":"merge","description":"Merge the working branch into main after a passing review.","budget":2.5,"allowed_tools":"merge"},
        {"name":"dream","description":"Explore the codebase around the completed work. Post new task suggestions as observations — improvements, refactors, follow-up features, or tech debt worth addressing.","model":"claude-sonnet-4-6","budget":4.0,"allowed_tools":"readonly"}
    ]'::jsonb,
    ARRAY['creative', 'continuous-improvement'],
    '{}'::jsonb,
    'ready',
    '00000000-0000-0000-0000-000000000000'
);
