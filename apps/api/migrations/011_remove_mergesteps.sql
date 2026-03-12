-- Update the default global playbooks seeded in 006 to remove merge steps.
UPDATE diraigent.playbook AS playbook
SET
    trigger_description = updated.trigger_description,
    steps = updated.steps,
    tags = updated.tags,
    metadata = updated.metadata,
    initial_state = updated.initial_state
FROM (
    VALUES
        (
            'Standard Lifecycle',
            'implement → review',
            '[
              {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
              {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"}
            ]'::jsonb,
            ARRAY['default']::text[],
            '{"git_strategy":"merge_to_default"}'::jsonb,
            'ready'
        ),
        (
            'Standard (Backlog Start)',
            'backlog → ready → implement → review',
            '[
              {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
              {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"}
            ]'::jsonb,
            ARRAY['default', 'backlog']::text[],
            '{"start_in_backlog":true,"git_strategy":"merge_to_default"}'::jsonb,
            'backlog'
        ),
        (
            'Researcher',
            'scope → gather → synthesize → document',
            '[
              {"name":"scope","description":"Read existing docs and codebase to define the research question. Post the refined scope as a decision.","model":"claude-sonnet-4-6","budget":5.0,"allowed_tools":"readonly"},
              {"name":"gather","description":"Fetch and aggregate relevant sources — code, docs, web references. Post key findings as knowledge entries.","model":"claude-sonnet-4-6","budget":10.0,"allowed_tools":"full"},
              {"name":"synthesize","description":"Synthesize findings into structured notes. Identify patterns, trade-offs, and recommendations.","model":"claude-opus-4-6","budget":15.0,"allowed_tools":"full"},
              {"name":"document","description":"Write the final documentation artifact. Post it as a task artifact.","budget":8.0,"allowed_tools":"full"}
            ]'::jsonb,
            ARRAY['research', 'documentation']::text[],
            '{}'::jsonb,
            'ready'
        ),
        (
            'Dreamer',
            'implement → review → dream',
            '[
              {"name":"implement","description":"Implement the feature or fix described in the task spec. Write tests where appropriate.","budget":12.0,"allowed_tools":"full"},
              {"name":"review","description":"Review the implementation for correctness, style, and test coverage. Post observations for any issues found.","budget":5.0,"allowed_tools":"readonly"},
              {"name":"dream","description":"Explore the codebase around the completed work. Post new task suggestions as observations — improvements, refactors, follow-up features, or tech debt worth addressing.","model":"claude-sonnet-4-6","budget":4.0,"allowed_tools":"readonly"}
            ]'::jsonb,
            ARRAY['creative', 'continuous-improvement']::text[],
            '{"git_strategy":"merge_to_default"}'::jsonb,
            'ready'
        )
) AS updated(title, trigger_description, steps, tags, metadata, initial_state)
WHERE playbook.title = updated.title
  AND playbook.tenant_id IS NULL
  AND playbook.created_by = '00000000-0000-0000-0000-000000000000';
