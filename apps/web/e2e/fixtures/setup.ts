/**
 * Shared Playwright mock setup for all screenshot/testing specs.
 * Intercepts API calls and returns realistic mock data.
 */
import type { Page, Route } from '@playwright/test';
import * as mock from './mock-data';

export const API = 'http://localhost:8082/v1';
export const PROJECT_ID = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';

export async function setupMocks(page: Page) {
  await page.route(new RegExp(`localhost:8082`), (route: Route) => {
    const url = route.request().url();
    const path = new URL(url).pathname.replace('/v1/', '').replace('/v1', '');
    const search = new URL(url).search;

    // Config
    if (path === 'config') {
      return route.fulfill({ json: mock.config });
    }

    // Projects
    if (path === '' || path === 'projects') {
      return route.fulfill({ json: mock.projects });
    }
    if (path === PROJECT_ID || path === `projects/${PROJECT_ID}`) {
      return route.fulfill({ json: mock.projects[0] });
    }

    // Settings
    if (path === 'settings') {
      return route.fulfill({ json: { projects_path: '/projects', repo_root: '/projects' } });
    }

    // Tasks with blockers (must be before general tasks)
    if (path === `${PROJECT_ID}/tasks/with-blockers`) {
      return route.fulfill({ json: mock.blockerTasks });
    }

    // Tasks
    if (path.match(new RegExp(`^${PROJECT_ID}/tasks`))) {
      if (search.includes('state=human_review')) {
        return route.fulfill({ json: { data: mock.reviewTasks, total: mock.reviewTasks.length, limit: 100, offset: 0, has_more: false } });
      }
      return route.fulfill({ json: mock.tasks });
    }

    // Task updates
    if (path.match(/^tasks\/.*\/updates/)) {
      return route.fulfill({ json: mock.taskUpdates });
    }

    // Task comments
    if (path.match(/^tasks\/.*\/comments/)) {
      return route.fulfill({ json: [] });
    }

    // Task dependencies
    if (path.match(/^tasks\/.*\/dependencies/)) {
      return route.fulfill({ json: { upstream: [], downstream: [] } });
    }

    // Task branch status
    if (path.includes(`${PROJECT_ID}/git/task-branch`)) {
      return route.fulfill({ json: { branch: 'agent/task-xxx', exists: true, is_pushed: true, ahead_remote: 0, behind_remote: 0, last_commit: 'abc', last_commit_message: 'fix', behind_default: 0, has_conflict: false } });
    }

    // Metrics
    if (path.match(new RegExp(`^${PROJECT_ID}/metrics`))) {
      return route.fulfill({ json: mock.metrics });
    }

    // Work items
    if (path === `${PROJECT_ID}/work` || path.match(new RegExp(`^${PROJECT_ID}/work$`))) {
      return route.fulfill({ json: mock.workItems.map((w: Record<string, unknown>) => w.item) });
    }
    if (path.match(new RegExp(`^${PROJECT_ID}/work/counts`))) {
      return route.fulfill({ json: { active: 3, ready: 1, processing: 0, achieved: 1, paused: 0, abandoned: 0 } });
    }
    if (path.match(/^work\/(.+)\/progress$/)) {
      const workId = path.match(/^work\/(.+)\/progress$/)![1];
      const w = mock.workItems.find((w: Record<string, unknown>) => (w.item as Record<string, unknown>).id === workId);
      return route.fulfill({ json: w ? w.progress : { total_tasks: 0, done_tasks: 0, percentage: 0 } });
    }
    if (path.match(/^work\/(.+)\/stats$/)) {
      const workId = path.match(/^work\/(.+)\/stats$/)![1];
      const w = mock.workItems.find((w: Record<string, unknown>) => (w.item as Record<string, unknown>).id === workId);
      return route.fulfill({ json: w ? w.stats : {} });
    }
    if (path.match(/^work\/(.+)\/children$/)) {
      return route.fulfill({ json: [] });
    }
    if (path.match(/^work\/(.+)\/tasks/)) {
      return route.fulfill({ json: mock.tasks.data.slice(0, 3) });
    }

    // Git
    if (path === `${PROJECT_ID}/git/main-status`) {
      return route.fulfill({ json: mock.mainPushStatus });
    }
    if (path === `${PROJECT_ID}/git/branches`) {
      return route.fulfill({ json: mock.branches });
    }

    // Playbooks
    if (path.match(/^playbooks\/(.+)/)) {
      const pbId = path.match(/^playbooks\/(.+)/)![1];
      const pb = mock.playbooks.find((p: Record<string, unknown>) => p.id === pbId);
      return route.fulfill({ json: pb || mock.playbooks[0] });
    }
    if (path === 'playbooks' || path.match(/^playbooks$/)) {
      return route.fulfill({ json: mock.playbooks });
    }
    if (path === `${PROJECT_ID}/step-templates`) {
      return route.fulfill({ json: mock.stepTemplates });
    }
    if (path === 'git-strategies') {
      return route.fulfill({ json: mock.gitStrategies });
    }

    // Observations
    if (path.match(new RegExp(`^${PROJECT_ID}/observations`))) {
      return route.fulfill({ json: mock.observations });
    }

    // Decisions
    if (path.match(new RegExp(`^${PROJECT_ID}/decisions`))) {
      return route.fulfill({ json: mock.decisions });
    }

    // Knowledge
    if (path.match(new RegExp(`^${PROJECT_ID}/knowledge`))) {
      return route.fulfill({ json: mock.knowledgeEntries });
    }

    // Integrations
    if (path.match(new RegExp(`^${PROJECT_ID}/integrations`))) {
      return route.fulfill({ json: mock.integrations });
    }

    // CI / Pipelines
    if (path.match(new RegExp(`^${PROJECT_ID}/ci/runs`))) {
      return route.fulfill({ json: mock.pipelineRuns });
    }

    // Verifications
    if (path.match(new RegExp(`^${PROJECT_ID}/verifications`))) {
      return route.fulfill({ json: mock.verifications });
    }

    // Audit
    if (path.match(new RegExp(`^${PROJECT_ID}/audit`))) {
      return route.fulfill({ json: mock.auditEntries });
    }

    // Reports
    if (path.match(new RegExp(`^${PROJECT_ID}/reports`))) {
      return route.fulfill({ json: mock.reports });
    }

    // Source tree
    if (path.match(new RegExp(`^${PROJECT_ID}/source/tree`))) {
      return route.fulfill({ json: mock.sourceTree });
    }

    // Team / Roles / Members
    if (path.match(new RegExp(`^${PROJECT_ID}/team/roles`))) {
      return route.fulfill({ json: mock.roles });
    }
    if (path.match(new RegExp(`^${PROJECT_ID}/members`))) {
      return route.fulfill({ json: [] });
    }

    // Provider configs
    if (path.match(new RegExp(`^${PROJECT_ID}/provider-configs`))) {
      return route.fulfill({ json: [] });
    }

    // Event rules
    if (path.match(new RegExp(`^${PROJECT_ID}/event-rules`))) {
      return route.fulfill({ json: [] });
    }

    // Tenant
    if (path === 'tenants/me') {
      return route.fulfill({ json: mock.tenant });
    }

    // Roles / Members (non-project-scoped)
    if (path === 'roles') {
      return route.fulfill({ json: mock.roles });
    }
    if (path === 'members') {
      return route.fulfill({ json: [] });
    }

    // Claude MD
    if (path.match(/claude-md$/)) {
      return route.fulfill({ json: { content: '# Project Instructions\n\nUse `cargo test` before committing.', exists: true } });
    }

    // Providers
    if (path.match(/providers$/)) {
      return route.fulfill({ json: [] });
    }

    // Agents
    if (path.match(/^agents/)) {
      return route.fulfill({ json: mock.agents });
    }

    // Chat
    if (path.match(new RegExp(`^${PROJECT_ID}/chat`))) {
      return route.abort();
    }

    // Packages
    if (path === 'packages') {
      return route.fulfill({ json: [] });
    }

    // Health
    if (path === 'health/ready' || url.includes('/health/live')) {
      return route.fulfill({ json: { status: 'ok' } });
    }

    console.log(`[mock] unhandled: ${route.request().method()} ${url} (path: ${path})`);
    return route.fulfill({ json: [] });
  });

  // Set theme + project in localStorage before navigation
  await page.addInitScript(() => {
    localStorage.setItem('zivue-theme', 'catppuccin-mocha');
    localStorage.setItem('zivue-accent', 'blue');
    localStorage.setItem('diraigent-project', 'a1b2c3d4-e5f6-7890-abcd-ef1234567890');
    localStorage.setItem('diraigent-chat-collapsed', 'true');

    const originalFetch = window.fetch;
    window.fetch = function (input: RequestInfo | URL, init?: RequestInit) {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
      if (url.endsWith('/config')) {
        return Promise.resolve(new Response(JSON.stringify({
          auth_required: false,
          chat_model: 'sonnet',
          api_version: 'v20260315-0100',
        }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
      }
      return originalFetch.call(window, input, init);
    };
  });
}
