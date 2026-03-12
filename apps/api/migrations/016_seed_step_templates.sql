-- Seed global step templates (tenant_id = NULL → visible to all tenants).
-- These match the steps currently used in the default playbooks.

INSERT INTO diraigent.step_template (tenant_id, name, description, model, budget, allowed_tools, context_level, on_complete, retriable, created_by)
VALUES
-- implement: the core coding step
(
    NULL,
    'implement',
    'Implement the feature or fix described in the task spec. Write tests where appropriate.',
    NULL,
    12.0,
    'full',
    'full',
    'next',
    true,
    '00000000-0000-0000-0000-000000000000'
),
-- review: code review step
(
    NULL,
    'review',
    'Review the implementation for correctness, style, and test coverage. Post observations for any issues found.',
    'claude-sonnet-4-6',
    5.0,
    'readonly',
    'minimal',
    'next',
    false,
    '00000000-0000-0000-0000-000000000000'
),
-- dream: creative exploration step
(
    NULL,
    'dream',
    'Explore the codebase around the completed work. Post new task suggestions as observations — improvements, refactors, follow-up features, or tech debt worth addressing.',
    'claude-sonnet-4-6',
    4.0,
    'readonly',
    'dream',
    'done',
    false,
    '00000000-0000-0000-0000-000000000000'
),
-- scope: research scoping step
(
    NULL,
    'scope',
    'Read existing docs and codebase to define the research question. Post the refined scope as a decision.',
    'claude-sonnet-4-6',
    5.0,
    'readonly',
    NULL,
    'next',
    false,
    '00000000-0000-0000-0000-000000000000'
),
-- gather: research gathering step
(
    NULL,
    'gather',
    'Fetch and aggregate relevant sources — code, docs, web references. Post key findings as knowledge entries.',
    'claude-sonnet-4-6',
    10.0,
    'full',
    NULL,
    'next',
    false,
    '00000000-0000-0000-0000-000000000000'
),
-- synthesize: research synthesis step
(
    NULL,
    'synthesize',
    'Synthesize findings into structured notes. Identify patterns, trade-offs, and recommendations.',
    'claude-opus-4-6',
    15.0,
    'full',
    NULL,
    'next',
    false,
    '00000000-0000-0000-0000-000000000000'
),
-- document: documentation writing step
(
    NULL,
    'document',
    'Write the final documentation artifact. Post it as a task artifact.',
    NULL,
    8.0,
    'full',
    NULL,
    'done',
    false,
    '00000000-0000-0000-0000-000000000000'
);
