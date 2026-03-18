/** Realistic mock data for Playwright screenshots. */

const PROJECT_ID = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';
const AGENT_ID = '11111111-2222-3333-4444-555555555555';

export const config = {
  auth_required: false,
  chat_model: 'sonnet',
  api_version: 'v20260315-0100',
};

export const projects = [
  {
    id: PROJECT_ID,
    name: 'Acme Platform',
    slug: 'acme-platform',
    description: 'Main product platform',
    parent_id: null,
    default_playbook_id: 'pb-001',
    repo_url: 'https://github.com/acme/platform',
    repo_path: '/projects/acme-platform',
    default_branch: 'main',
    service_name: null,
    metadata: {},
    created_at: '2026-02-01T10:00:00Z',
    updated_at: '2026-03-15T08:00:00Z',
    git_mode: 'standalone',
    git_root: '/projects/acme-platform',
    project_root: '/projects/acme-platform',
    resolved_path: '/projects/acme-platform',
    git_resolved_path: '/projects/acme-platform',
  },
];

export const metrics = {
  project_id: PROJECT_ID,
  range_days: 30,
  task_summary: {
    total: 47,
    done: 31,
    cancelled: 2,
    in_progress: 6,
    ready: 5,
    backlog: 3,
    human_review: 0,
  },
  tasks_per_day: Array.from({ length: 14 }, (_, i) => ({
    day: `2026-03-${String(i + 1).padStart(2, '0')}`,
    count: Math.floor(Math.random() * 5) + 1,
  })),
  avg_time_in_state_hours: [
    { state: 'ready', avg_hours: 0.3 },
    { state: 'implement', avg_hours: 1.2 },
    { state: 'review', avg_hours: 0.5 },
    { state: 'done', avg_hours: null },
  ],
  agent_breakdown: [
    { agent_id: AGENT_ID, agent_name: 'claude-agent-1', tasks_completed: 18, tasks_in_progress: 3, avg_completion_hours: 0.8 },
    { agent_id: '22222222-3333-4444-5555-666666666666', agent_name: 'claude-agent-2', tasks_completed: 13, tasks_in_progress: 3, avg_completion_hours: 1.1 },
  ],
  playbook_completion: [
    { playbook_id: 'pb-001', playbook_title: 'Standard Development', total_tasks: 35, completed_tasks: 28, completion_rate: 80.0 },
    { playbook_id: 'pb-002', playbook_title: 'Research Spike', total_tasks: 12, completed_tasks: 3, completion_rate: 25.0 },
  ],
  cost_summary: {
    total_input_tokens: 45_200_000,
    total_output_tokens: 12_800_000,
    total_cost_usd: 38.42,
  },
  task_costs: [],
  tokens_per_day: Array.from({ length: 14 }, (_, i) => ({
    day: `2026-03-${String(i + 1).padStart(2, '0')}`,
    input_tokens: Math.floor(Math.random() * 4_000_000) + 1_000_000,
    output_tokens: Math.floor(Math.random() * 1_200_000) + 300_000,
    cost_usd: parseFloat((Math.random() * 4 + 1).toFixed(2)),
  })),
};

export const tasks = {
  data: [
    mkTask(1, 'Add user authentication flow', 'feature', 'implement', true),
    mkTask(2, 'Fix payment webhook retry logic', 'bug', 'implement', false),
    mkTask(3, 'Refactor database connection pooling', 'refactor', 'review', false),
    mkTask(4, 'Add OpenTelemetry tracing to API', 'feature', 'ready', false),
    mkTask(5, 'Write integration tests for billing module', 'test', 'ready', true),
    mkTask(6, 'Migrate to new email provider SDK', 'chore', 'implement', false),
    mkTask(7, 'Add rate limiting per API key', 'feature', 'implement', false),
    mkTask(8, 'Document deployment runbook', 'docs', 'done', false),
    mkTask(9, 'Optimize image upload pipeline', 'feature', 'implement', false),
    mkTask(10, 'Fix timezone handling in scheduler', 'bug', 'ready', false),
    mkTask(11, 'Add CSV export to reports', 'feature', 'implement', false),
    mkTask(12, 'Update dependency versions', 'chore', 'done', false),
  ],
  total: 12,
  limit: 200,
  offset: 0,
  has_more: false,
};

export const workItems = [
  mkWork('w1', 'User Authentication System', 'epic', 'active', 8, 3),
  mkWork('w2', 'Payment Processing v2', 'feature', 'active', 5, 2),
  mkWork('w3', 'Q1 Performance Improvements', 'milestone', 'active', 12, 9),
  mkWork('w4', 'Developer Experience Sprint', 'sprint', 'active', 6, 1),
  mkWork('w5', 'API Documentation Overhaul', 'feature', 'achieved', 4, 4),
];

export const reviewTasks = [
  mkTask(3, 'Refactor database connection pooling', 'refactor', 'human_review', false),
  mkTask(13, 'Add webhook signature validation', 'feature', 'human_review', false),
  mkTask(14, 'Fix race condition in queue processor', 'bug', 'human_review', true),
];

export const blockerTasks = [
  {
    ...mkTask(15, 'Migrate user sessions to Redis', 'feature', 'implement', false),
    _blockerUpdates: [
      { kind: 'blocker', content: 'Redis cluster connection fails intermittently. Need to check TLS cert rotation.' },
    ],
  },
];

export const taskUpdates = [
  { id: 'u1', task_id: '', agent_id: AGENT_ID, user_id: null, kind: 'progress', content: 'Implemented core auth middleware with JWT validation', metadata: {}, created_at: '2026-03-14T14:30:00Z' },
  { id: 'u2', task_id: '', agent_id: AGENT_ID, user_id: null, kind: 'artifact', content: '```rust\npub async fn auth_middleware(req: Request) -> Result<Request> {\n    let token = extract_bearer(&req)?;\n    let claims = validate_jwt(token).await?;\n    Ok(req.with_extension(claims))\n}\n```', metadata: {}, created_at: '2026-03-14T14:45:00Z' },
  { id: 'u3', task_id: '', agent_id: AGENT_ID, user_id: null, kind: 'progress', content: 'All 12 tests passing. Ready for review.', metadata: {}, created_at: '2026-03-14T15:00:00Z' },
];

export const observations = [
  { id: 'obs1', project_id: PROJECT_ID, kind: 'risk', title: 'No rate limiting on auth endpoints', description: 'Login and token refresh endpoints have no rate limiting. Could be exploited for brute-force attacks.', severity: 'high', source_task_id: null, acknowledged: false, created_by: AGENT_ID, created_at: '2026-03-14T12:00:00Z', updated_at: '2026-03-14T12:00:00Z' },
  { id: 'obs2', project_id: PROJECT_ID, kind: 'improvement', title: 'Connection pool size should be configurable', description: 'Currently hardcoded to 10. Should read from environment variable.', severity: 'medium', source_task_id: null, acknowledged: false, created_by: AGENT_ID, created_at: '2026-03-13T16:00:00Z', updated_at: '2026-03-13T16:00:00Z' },
  { id: 'obs3', project_id: PROJECT_ID, kind: 'insight', title: 'Batch processing pattern reduces API calls by 60%', description: 'Grouping webhook deliveries into batches of 50 reduces external API calls significantly.', severity: 'info', source_task_id: null, acknowledged: false, created_by: AGENT_ID, created_at: '2026-03-12T09:00:00Z', updated_at: '2026-03-12T09:00:00Z' },
];

export const decisions = [
  { id: 'dec1', project_id: PROJECT_ID, title: 'Use JWT with short-lived tokens + refresh', status: 'accepted', rationale: 'Better security posture than long-lived tokens. Refresh tokens allow session management.', alternatives: 'Session cookies, API keys', superseded_by: null, created_by: AGENT_ID, created_at: '2026-03-10T10:00:00Z', updated_at: '2026-03-10T10:00:00Z' },
];

export const playbooks = [
  {
    id: 'pb-001',
    tenant_id: null,
    title: 'Standard Development',
    trigger_description: 'Use for all feature and bug fix tasks',
    steps: [
      { name: 'implement', description: 'Write the code changes', on_complete: '', step: 0, model: 'sonnet', budget: 5.0, allowed_tools: 'full' },
      { name: 'review', description: 'Review the implementation for quality', on_complete: '', step: 1, model: 'opus', budget: 2.0, allowed_tools: 'readonly' },
      { name: 'merge', description: 'Merge the changes to the default branch', on_complete: '', step: 2, git_action: 'merge' },
    ],
    tags: ['default', 'development'],
    initial_state: 'ready',
    metadata: { git_strategy: 'merge_to_default' },
    created_at: '2026-02-01T10:00:00Z',
    created_by: 'system',
    updated_at: '2026-03-01T10:00:00Z',
  },
  {
    id: 'pb-002',
    tenant_id: null,
    title: 'Research Spike',
    trigger_description: 'For exploratory tasks that need investigation before implementation',
    steps: [
      { name: 'dream', description: 'Research and propose an approach', on_complete: '', step: 0, model: 'opus', budget: 3.0, allowed_tools: 'readonly', context_level: 'dream' },
      { name: 'implement', description: 'Implement the proposed approach', on_complete: '', step: 1, model: 'sonnet', budget: 5.0, allowed_tools: 'full' },
      { name: 'review', description: 'Review the implementation', on_complete: '', step: 2, model: 'opus', budget: 2.0, allowed_tools: 'readonly' },
    ],
    tags: ['research', 'spike'],
    initial_state: 'ready',
    metadata: { git_strategy: 'merge_to_default' },
    created_at: '2026-02-01T10:00:00Z',
    created_by: 'system',
    updated_at: '2026-03-01T10:00:00Z',
  },
];

export const stepTemplates = [
  { id: 'st1', tenant_id: null, name: 'implement', description: 'Write code changes following the task spec', model: 'sonnet', budget: 5.0, allowed_tools: 'full', context_level: 'full', on_complete: null, retriable: true, max_cycles: 3, timeout_minutes: null, mcp_servers: null, agents: null, agent: null, settings: null, env: null, vars: null, tags: ['core'], metadata: {}, created_by: 'system', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-02-01T10:00:00Z' },
  { id: 'st2', tenant_id: null, name: 'review', description: 'Review implementation for quality and correctness', model: 'opus', budget: 2.0, allowed_tools: 'readonly', context_level: 'full', on_complete: null, retriable: false, max_cycles: null, timeout_minutes: null, mcp_servers: null, agents: null, agent: null, settings: null, env: null, vars: null, tags: ['core'], metadata: {}, created_by: 'system', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-02-01T10:00:00Z' },
  { id: 'st3', tenant_id: null, name: 'dream', description: 'Research and propose an approach before implementation', model: 'opus', budget: 3.0, allowed_tools: 'readonly', context_level: 'dream', on_complete: null, retriable: false, max_cycles: null, timeout_minutes: null, mcp_servers: null, agents: null, agent: null, settings: null, env: null, vars: null, tags: ['research'], metadata: {}, created_by: 'system', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-02-01T10:00:00Z' },
];

export const gitStrategies = [
  { id: 'merge_to_default', name: 'Merge to default', description: 'Merge task branch to the default branch on completion' },
  { id: 'branch_only', name: 'Branch only', description: 'Create a branch but do not merge automatically' },
  { id: 'feature_branch', name: 'Feature branch (per goal)', description: 'Tasks branch from a goal branch and merge back into it' },
  { id: 'no_git', name: 'No git', description: 'No git operations' },
];

export const mainPushStatus = {
  ahead: 3,
  behind: 0,
  last_commit: 'abc1234',
  last_commit_message: 'fix: resolve payment webhook retry logic',
};

export const branches = {
  current_branch: 'main',
  branches: [
    { name: 'main', commit: 'abc1234', is_pushed: true, ahead_remote: 3, behind_remote: 0, task_id_prefix: null },
    { name: 'agent/task-a1b2c3d4', commit: 'def5678', is_pushed: true, ahead_remote: 0, behind_remote: 0, task_id_prefix: 'a1b2c3d4' },
  ],
};

// ── Helpers ──

function mkTask(num: number, title: string, kind: string, state: string, urgent: boolean) {
  const id = `task-${String(num).padStart(4, '0')}-0000-0000-0000-000000000000`;
  return {
    id,
    project_id: PROJECT_ID,
    number: num,
    title,
    kind,
    state,
    urgent,
    context: { spec: `Implement ${title.toLowerCase()}`, files: ['src/main.rs'], test_cmd: 'cargo test', acceptance_criteria: [`${title} works correctly`, 'All tests pass'] },
    assigned_agent_id: state === 'implement' || state === 'review' ? AGENT_ID : null,
    claimed_at: state === 'implement' || state === 'review' ? '2026-03-14T10:00:00Z' : null,
    required_capabilities: [],
    assigned_role_id: null,
    delegated_by: null,
    delegated_at: null,
    playbook_id: 'pb-001',
    playbook_step: state === 'implement' ? 0 : state === 'review' ? 1 : null,
    decision_id: null,
    created_by: 'user',
    created_at: '2026-03-14T09:00:00Z',
    updated_at: '2026-03-14T15:00:00Z',
    completed_at: state === 'done' ? '2026-03-14T16:00:00Z' : null,
    reverted_at: null,
    flagged: false,
    parent_id: null,
    decision: null,
    input_tokens: Math.floor(Math.random() * 2_000_000) + 100_000,
    output_tokens: Math.floor(Math.random() * 500_000) + 50_000,
    cost_usd: parseFloat((Math.random() * 3 + 0.5).toFixed(2)),
  };
}

export const knowledgeEntries = [
  { id: 'k1', project_id: PROJECT_ID, title: 'Authentication Flow', category: 'pattern', content: 'JWT with RS256 signing. Access tokens expire after 15 minutes, refresh tokens after 7 days. Token rotation on each refresh.', tags: ['auth', 'security'], source: 'agent', source_task_id: null, created_by: AGENT_ID, created_at: '2026-03-10T10:00:00Z', updated_at: '2026-03-10T10:00:00Z' },
  { id: 'k2', project_id: PROJECT_ID, title: 'Database Migration Convention', category: 'convention', content: 'Migrations use sequential numbering (001_, 002_). Always include a down migration. Test against a fresh database before merging.', tags: ['database', 'conventions'], source: 'agent', source_task_id: null, created_by: AGENT_ID, created_at: '2026-03-08T14:00:00Z', updated_at: '2026-03-08T14:00:00Z' },
  { id: 'k3', project_id: PROJECT_ID, title: 'API Error Response Format', category: 'convention', content: 'All API errors return { "error": { "code": "...", "message": "...", "details": {} } }. HTTP status codes follow REST conventions.', tags: ['api', 'conventions'], source: 'agent', source_task_id: null, created_by: AGENT_ID, created_at: '2026-03-05T09:00:00Z', updated_at: '2026-03-05T09:00:00Z' },
  { id: 'k4', project_id: PROJECT_ID, title: 'Module: apps/api', category: 'architecture', content: 'Rust/Axum API server. Routes in src/routes/, repository pattern in src/repository.rs. AppState holds DB pool and config.', tags: ['architecture'], source: 'codegen', source_task_id: null, created_by: AGENT_ID, created_at: '2026-03-01T10:00:00Z', updated_at: '2026-03-14T10:00:00Z' },
];

export const integrations = [
  { id: 'int1', project_id: PROJECT_ID, name: 'Slack Notifications', kind: 'webhook', config: { url: 'https://hooks.slack.com/...', events: ['task.done', 'work.achieved'] }, enabled: true, created_at: '2026-03-01T10:00:00Z', updated_at: '2026-03-10T10:00:00Z' },
  { id: 'int2', project_id: PROJECT_ID, name: 'GitHub Mirror', kind: 'git_mirror', config: { remote: 'git@github.com:acme/platform.git' }, enabled: true, created_at: '2026-02-15T10:00:00Z', updated_at: '2026-03-12T10:00:00Z' },
];

export const pipelineRuns = {
  data: [
    { id: 'run1', project_id: PROJECT_ID, provider: 'forgejo', branch: 'agent/task-0001', status: 'success', commit_sha: 'abc1234', commit_message: 'Add user auth flow', started_at: '2026-03-14T14:00:00Z', finished_at: '2026-03-14T14:05:00Z', duration_seconds: 300, url: null, metadata: {} },
    { id: 'run2', project_id: PROJECT_ID, provider: 'forgejo', branch: 'agent/task-0002', status: 'failure', commit_sha: 'def5678', commit_message: 'Fix payment webhook', started_at: '2026-03-14T15:00:00Z', finished_at: '2026-03-14T15:03:00Z', duration_seconds: 180, url: null, metadata: {} },
    { id: 'run3', project_id: PROJECT_ID, provider: 'forgejo', branch: 'main', status: 'success', commit_sha: 'ghi9012', commit_message: 'Merge: refactor db pooling', started_at: '2026-03-13T10:00:00Z', finished_at: '2026-03-13T10:08:00Z', duration_seconds: 480, url: null, metadata: {} },
  ],
  total: 3,
  page: 1,
  per_page: 20,
};

export const verifications = [
  { id: 'ver1', task_id: 'task-0001-0000-0000-0000-000000000000', project_id: PROJECT_ID, title: 'CI Pipeline', kind: 'ci', status: 'pass', detail: 'All 47 tests passed', run_id: 'run1', created_at: '2026-03-14T14:05:00Z', updated_at: '2026-03-14T14:05:00Z' },
  { id: 'ver2', task_id: 'task-0002-0000-0000-0000-000000000000', project_id: PROJECT_ID, title: 'CI Pipeline', kind: 'ci', status: 'fail', detail: '3 tests failed in billing module', run_id: 'run2', created_at: '2026-03-14T15:03:00Z', updated_at: '2026-03-14T15:03:00Z' },
  { id: 'ver3', task_id: 'task-0003-0000-0000-0000-000000000000', project_id: PROJECT_ID, title: 'Code Review', kind: 'review', status: 'pass', detail: 'Approved by reviewer agent', run_id: null, created_at: '2026-03-14T12:00:00Z', updated_at: '2026-03-14T12:00:00Z' },
];

export const auditEntries = [
  { id: 'aud1', project_id: PROJECT_ID, entity_type: 'task', entity_id: 'task-0001-0000-0000-0000-000000000000', action: 'transition', actor_id: AGENT_ID, actor_type: 'agent', detail: { from: 'ready', to: 'implement' }, created_at: '2026-03-14T10:00:00Z' },
  { id: 'aud2', project_id: PROJECT_ID, entity_type: 'task', entity_id: 'task-0003-0000-0000-0000-000000000000', action: 'transition', actor_id: AGENT_ID, actor_type: 'agent', detail: { from: 'implement', to: 'done' }, created_at: '2026-03-14T11:30:00Z' },
  { id: 'aud3', project_id: PROJECT_ID, entity_type: 'work', entity_id: 'w1', action: 'create', actor_id: null, actor_type: 'user', detail: { title: 'User Authentication System' }, created_at: '2026-03-13T09:00:00Z' },
  { id: 'aud4', project_id: PROJECT_ID, entity_type: 'task', entity_id: 'task-0008-0000-0000-0000-000000000000', action: 'transition', actor_id: AGENT_ID, actor_type: 'agent', detail: { from: 'review', to: 'done' }, created_at: '2026-03-14T16:00:00Z' },
  { id: 'aud5', project_id: PROJECT_ID, entity_type: 'playbook', entity_id: 'pb-001', action: 'update', actor_id: null, actor_type: 'user', detail: { field: 'steps' }, created_at: '2026-03-12T14:00:00Z' },
];

export const reports = [
  { id: 'rpt1', project_id: PROJECT_ID, title: 'Weekly Sprint Report — Week 11', status: 'published', kind: 'sprint', content: '## Summary\n\n31 tasks completed, 6 in progress. Authentication system 80% complete.', metadata: {}, created_by: AGENT_ID, created_at: '2026-03-14T18:00:00Z', updated_at: '2026-03-14T18:00:00Z' },
  { id: 'rpt2', project_id: PROJECT_ID, title: 'Architecture Review — API Layer', status: 'draft', kind: 'review', content: '## Findings\n\nThe API layer is well-structured with consistent patterns...', metadata: {}, created_by: AGENT_ID, created_at: '2026-03-13T10:00:00Z', updated_at: '2026-03-13T10:00:00Z' },
];

export const sourceTree = {
  entries: [
    { name: 'apps', kind: 'dir', path: 'apps' },
    { name: 'README.md', kind: 'file', path: 'README.md' },
    { name: 'CLAUDE.md', kind: 'file', path: 'CLAUDE.md' },
    { name: 'docker-compose.yml', kind: 'file', path: 'docker-compose.yml' },
  ],
};

export const roles = [
  { id: 'role1', project_id: PROJECT_ID, name: 'developer', authorities: ['execute', 'create'], description: 'Can implement and create tasks', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-02-01T10:00:00Z' },
  { id: 'role2', project_id: PROJECT_ID, name: 'reviewer', authorities: ['review', 'decide'], description: 'Can review work and make decisions', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-02-01T10:00:00Z' },
];

export const agents = [
  { id: AGENT_ID, name: 'claude-agent-1', status: 'online', capabilities: ['implement', 'review', 'dream'], last_heartbeat: '2026-03-14T15:00:00Z', created_at: '2026-02-01T10:00:00Z', updated_at: '2026-03-14T15:00:00Z' },
  { id: '22222222-3333-4444-5555-666666666666', name: 'claude-agent-2', status: 'online', capabilities: ['implement', 'review'], last_heartbeat: '2026-03-14T14:55:00Z', created_at: '2026-02-15T10:00:00Z', updated_at: '2026-03-14T14:55:00Z' },
];

export const tenant = {
  id: 'tenant-001',
  name: 'Acme Corp',
  slug: 'acme',
  settings: {},
  created_at: '2026-01-01T10:00:00Z',
  updated_at: '2026-03-01T10:00:00Z',
};

function mkWork(id: string, title: string, workType: string, status: string, total: number, done: number) {
  return {
    item: {
      id,
      project_id: PROJECT_ID,
      title,
      description: `${title} — high-level goal`,
      status,
      work_type: workType,
      priority: 1,
      parent_work_id: null,
      auto_status: true,
      success_criteria: '',
      metadata: {},
      created_at: '2026-03-01T10:00:00Z',
      created_by: 'user',
      updated_at: '2026-03-14T15:00:00Z',
    },
    progress: {
      total_tasks: total,
      done_tasks: done,
      percentage: Math.round((done / total) * 100),
    },
    stats: {
      work_id: id,
      backlog_count: 0,
      ready_count: Math.max(0, total - done - 2),
      working_count: Math.min(2, total - done),
      done_count: done,
      cancelled_count: 0,
      total_count: total,
      kind_breakdown: { feature: Math.ceil(total * 0.6), bug: Math.floor(total * 0.2), chore: Math.floor(total * 0.2) },
      total_cost_usd: parseFloat((done * 2.5).toFixed(2)),
      total_input_tokens: done * 3_000_000,
      total_output_tokens: done * 800_000,
      blocked_count: 0,
      avg_completion_hours: 1.2,
      oldest_open_task_date: '2026-03-10T10:00:00Z',
    },
  };
}
