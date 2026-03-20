/**
 * Playwright screenshot spec for Diraigent landing page images.
 *
 * Usage:
 *   cd apps/web
 *   npm run e2e:landing
 *
 * Screenshots are saved to ../landing/public/assets/images/ as both PNG and WebP.
 */
import { test, type Page, type Route } from '@playwright/test';
import { execSync } from 'child_process';
import * as mock from './fixtures/mock-data';

const API = 'http://localhost:8082/v1';
const SCREENSHOT_DIR = '../landing/public/assets/images';
const PROJECT_ID = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';

/** Take a PNG screenshot and convert to WebP via cwebp. */
async function screenshot(page: Page, name: string) {
  const png = `${SCREENSHOT_DIR}/${name}.png`;
  await page.screenshot({ path: png, fullPage: false });
  try {
    execSync(`cwebp -q 80 "${png}" -o "${SCREENSHOT_DIR}/${name}.webp"`, { stdio: 'pipe' });
  } catch {
    console.warn(`cwebp not found — skipping WebP for ${name}`);
  }
}

async function setupMocks(page: Page) {
  // Single catch-all route handler for all API requests.
  // Using individual page.route() calls caused ordering issues where
  // the catch-all intercepted requests before specific routes.
  await page.route(new RegExp(`localhost:8082`), (route: Route) => {
    const url = route.request().url();
    const path = new URL(url).pathname.replace('/v1/', '').replace('/v1', '');
    const search = new URL(url).search;

    // Config
    if (path === 'config') {
      return route.fulfill({ json: mock.config });
    }

    // Projects — the API service uses /v1 as baseUrl, so project list is GET /v1
    // and project detail is GET /v1/{id} (no /projects/ prefix)
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

    // Task branch status
    if (path.includes(`${PROJECT_ID}/git/task-branch`)) {
      return route.fulfill({ json: { branch: 'agent/task-xxx', exists: true, is_pushed: true, ahead_remote: 0, behind_remote: 0, last_commit: 'abc', last_commit_message: 'fix', behind_default: 0, has_conflict: false } });
    }

    // Metrics
    if (path.match(new RegExp(`^${PROJECT_ID}/metrics`))) {
      return route.fulfill({ json: mock.metrics });
    }

    // Work items — API returns SpWork[] (just the item, not the wrapped {item,progress,stats})
    if (path === `${PROJECT_ID}/work` || path.match(new RegExp(`^${PROJECT_ID}/work$`))) {
      return route.fulfill({ json: mock.workItems.map((w: Record<string, unknown>) => w.item) });
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

    // Playbooks — individual playbook or list
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

    // Agents
    if (path.match(/^agents/)) {
      return route.fulfill({ json: [] });
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

    // Patch fetch to intercept the config call before Angular bootstraps.
    // page.route() may miss requests that fire during APP_INITIALIZER.
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

test.describe('Landing page screenshots', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  test('dashboard', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500); // let charts render
    await screenshot(page, 'screenshot-dashboard');
  });

  test('goals / work', async ({ page }) => {
    await page.goto('/work');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await screenshot(page, 'screenshot-goals');
  });

  test('review queue', async ({ page }) => {
    await page.goto('/review');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await screenshot(page, 'screenshot-review');
  });

  test('chat panel', async ({ page }) => {
    // Show chat expanded
    await page.addInitScript(() => {
      localStorage.setItem('diraigent-chat-collapsed', 'false');
    });
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    // Add some fake chat messages to localStorage
    await page.evaluate(() => {
      const pid = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';
      const messages = [
        { role: 'user', content: 'What tasks are currently blocked?' },
        { role: 'assistant', content: 'There is one blocked task:\n\n**#15 — Migrate user sessions to Redis** (feature, implement)\n\nBlocker: Redis cluster connection fails intermittently. The issue appears to be related to TLS certificate rotation. I recommend checking the cert expiry and renewal configuration.\n\nWould you like me to create a subtask to investigate the Redis TLS setup?' },
        { role: 'user', content: 'Yes, create that subtask and assign it to the research spike playbook.' },
        { role: 'assistant', content: 'Done. Created task **#16 — Investigate Redis TLS certificate rotation** with the Research Spike playbook. It\'s now in the ready queue and will be picked up by the next available agent.\n\nThe task includes:\n- Spec: Check TLS cert expiry, test renewal flow, document rotation procedure\n- Acceptance criteria: Redis connection stable after cert rotation\n- Parent: #15 (Migrate user sessions to Redis)' },
      ];
      localStorage.setItem(`diraigent-chat-${pid}`, JSON.stringify(messages));
    });

    // Reload to pick up the messages
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await screenshot(page, 'screenshot-chat');
  });

  test('playbook builder', async ({ page }) => {
    // Navigate to edit the first playbook
    await page.goto('/playbooks/pb-001/edit');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await screenshot(page, 'screenshot-playbook');
  });
});
