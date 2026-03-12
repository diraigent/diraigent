-- Seed data (default tenant + builtin packages)

INSERT INTO diraigent.tenant (id, name, slug)
VALUES ('00000000-0000-0000-0000-000000000001', 'Default', 'default');

INSERT INTO diraigent.package (slug, name, description, is_builtin,
    allowed_task_kinds, allowed_knowledge_categories, allowed_observation_kinds,
    allowed_event_kinds, allowed_integration_kinds)
VALUES
(
    'software-dev',
    'Software Development',
    'Standard software development workflow with features, bugs, refactoring, and standard observability kinds.',
    true,
    ARRAY['feature','bug','refactor','docs','test','research','chore','spike'],
    ARRAY['architecture','convention','pattern','anti_pattern','setup','general'],
    ARRAY['insight','risk','opportunity','smell','inconsistency','improvement'],
    ARRAY['ci','deploy','error','merge','release','alert','custom'],
    ARRAY['logging','tracing','metrics','git','ci','messaging','monitoring','storage','database','custom']
),
(
    'researcher',
    'Research',
    'Research-oriented workflow with hypotheses, experiments, analysis, and literature reviews.',
    true,
    ARRAY['hypothesis','experiment','analysis','literature-review','write-up','chore'],
    ARRAY['finding','method','reference','assumption','open-question'],
    ARRAY['insight','risk','anomaly','replication-failure','opportunity'],
    ARRAY['dataset-update','model-run','alert','custom'],
    ARRAY['storage','database','custom']
),
(
    'ops',
    'Operations',
    'Operations and SRE workflow with incidents, runbooks, hardening, and capacity planning.',
    true,
    ARRAY['incident','runbook','hardening','capacity','chore'],
    ARRAY['runbook','post-mortem','architecture','convention','general'],
    ARRAY['risk','incident','degradation','opportunity','improvement'],
    ARRAY['alert','deploy','error','release','custom'],
    ARRAY['logging','tracing','metrics','monitoring','storage','database','custom']
)
ON CONFLICT (slug) DO NOTHING;
