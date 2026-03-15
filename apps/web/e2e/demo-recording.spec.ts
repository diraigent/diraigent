/**
 * Playwright demo recording for README animated GIF.
 *
 * Records a scripted walkthrough of the Diraigent UI:
 *   Dashboard → Work (expand goal) → Review Queue → Chat → Playbook
 *
 * Usage:
 *   cd apps/web
 *   npx playwright test e2e/demo-recording.spec.ts
 *   # produces test-results/.../video.webm — copy it, then convert:
 *   ffmpeg -i video.webm -vf "fps=10,scale=960:-1:flags=lanczos" -loop 0 ../landing/public/assets/images/demo.gif
 */
import { test, type Page, type Route } from '@playwright/test';
import * as mock from './fixtures/mock-data';

const API = 'http://localhost:8082/v1';
const PROJECT_ID = 'a1b2c3d4-e5f6-7890-abcd-ef1234567890';

// Enable video recording at top level
test.use({
  video: { mode: 'on', size: { width: 1280, height: 800 } },
});

async function setupMocks(page: Page) {
  await page.route(new RegExp(`localhost:8082`), (route: Route) => {
    const url = route.request().url();
    const path = new URL(url).pathname.replace('/v1/', '').replace('/v1', '');
    const search = new URL(url).search;

    if (path === 'config') return route.fulfill({ json: mock.config });
    if (path === '' || path === 'projects') return route.fulfill({ json: mock.projects });
    if (path === PROJECT_ID || path === `projects/${PROJECT_ID}`) return route.fulfill({ json: mock.projects[0] });
    if (path === 'settings') return route.fulfill({ json: { projects_path: '/projects', repo_root: '/projects' } });
    if (path === `${PROJECT_ID}/tasks/with-blockers`) return route.fulfill({ json: mock.blockerTasks });
    if (path.match(new RegExp(`^${PROJECT_ID}/tasks`))) {
      if (search.includes('state=human_review')) {
        return route.fulfill({ json: { data: mock.reviewTasks, total: mock.reviewTasks.length, limit: 100, offset: 0, has_more: false } });
      }
      return route.fulfill({ json: mock.tasks });
    }
    if (path.match(/^tasks\/.*\/updates/)) return route.fulfill({ json: mock.taskUpdates });
    if (path.match(/^tasks\/.*\/comments/)) return route.fulfill({ json: [] });
    if (path.match(/^tasks\/.*\/dependencies/)) return route.fulfill({ json: { depends_on: [], blocks: [] } });
    if (path.match(/^tasks\/.*\/changed-files/)) return route.fulfill({ json: [] });
    if (path.includes(`${PROJECT_ID}/git/task-branch`)) {
      return route.fulfill({ json: { branch: 'agent/task-003', exists: true, is_pushed: true, ahead_remote: 0, behind_remote: 0, last_commit: 'abc123', last_commit_message: 'refactor connection pooling', behind_default: 0, has_conflict: false } });
    }
    if (path.match(new RegExp(`^${PROJECT_ID}/metrics`))) return route.fulfill({ json: mock.metrics });
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
    if (path.match(/^work\/(.+)\/children$/)) return route.fulfill({ json: [] });
    if (path.match(/^work\/(.+)\/tasks/)) return route.fulfill({ json: mock.tasks.data.slice(0, 3) });
    if (path === `${PROJECT_ID}/git/main-status`) return route.fulfill({ json: mock.mainPushStatus });
    if (path === `${PROJECT_ID}/git/branches`) return route.fulfill({ json: mock.branches });
    if (path.match(/^playbooks\/(.+)/)) {
      const pbId = path.match(/^playbooks\/(.+)/)![1];
      const pb = mock.playbooks.find((p: Record<string, unknown>) => p.id === pbId);
      return route.fulfill({ json: pb || mock.playbooks[0] });
    }
    if (path === 'playbooks' || path.match(/^playbooks$/)) return route.fulfill({ json: mock.playbooks });
    if (path === `${PROJECT_ID}/step-templates`) return route.fulfill({ json: mock.stepTemplates });
    if (path === 'git-strategies') return route.fulfill({ json: mock.gitStrategies });
    if (path.match(new RegExp(`^${PROJECT_ID}/observations`))) return route.fulfill({ json: mock.observations });
    if (path.match(new RegExp(`^${PROJECT_ID}/decisions`))) return route.fulfill({ json: mock.decisions });
    if (path.match(/^agents/)) return route.fulfill({ json: [] });
    if (path.match(new RegExp(`^${PROJECT_ID}/chat`))) return route.abort();
    if (path === 'health/ready' || url.includes('/health/live')) return route.fulfill({ json: { status: 'ok' } });
    if (path === 'packages') return route.fulfill({ json: [] });
    if (path.match(/^verifications/)) return route.fulfill({ json: [] });

    return route.fulfill({ json: [] });
  });

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

test.describe('Demo recording', () => {
  test('full walkthrough', async ({ page }) => {
    test.setTimeout(120_000); // 2 minutes for the full walkthrough

    await setupMocks(page);

    // ── Scene 1: Dashboard ──────────────────────────────
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(2500);

    // Scroll down to see the token chart
    await page.evaluate(() => window.scrollBy({ top: 350, behavior: 'smooth' }));
    await page.waitForTimeout(2000);
    await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'smooth' }));
    await page.waitForTimeout(1000);

    // ── Scene 2: Work / Goals ───────────────────────────
    await page.locator('a:has-text("Work")').click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    // Click first goal to expand it
    await page.locator('button.w-full.text-left').first().click();
    await page.waitForTimeout(2500);

    // ── Scene 3: Review Queue ───────────────────────────
    await page.locator('a:has-text("Review")').click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    // Click the expand chevron on the first review card
    const chevron = page.locator('.rounded-xl button.p-1\\.5, .rounded-xl button[class*="p-1"]').first();
    if (await chevron.isVisible({ timeout: 2000 }).catch(() => false)) {
      await chevron.click();
      await page.waitForTimeout(2500);
    } else {
      await page.waitForTimeout(1500);
    }

    // ── Scene 4: Chat ───────────────────────────────────
    // Pre-seed chat history
    await page.evaluate((pid) => {
      localStorage.setItem(`diraigent-chat-${pid}`, JSON.stringify([
        { role: 'user', content: 'What tasks are currently blocked?' },
        { role: 'assistant', content: 'There is one blocked task:\n\n**#15 — Migrate user sessions to Redis** (feature, implement)\n\nBlocker: Redis cluster connection fails intermittently due to TLS certificate rotation.\n\nWould you like me to create a subtask to investigate?' },
      ]));
      localStorage.removeItem('diraigent-chat-collapsed');
    }, PROJECT_ID);

    // Navigate to dashboard to reload with chat open
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    // Type a follow-up in the chat input
    const chatInput = page.locator('textarea[placeholder*="Ask"]');
    if (await chatInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await chatInput.click({ force: true });
      await page.waitForTimeout(300);
      const message = 'Yes, create a subtask for the TLS investigation';
      for (const char of message) {
        await page.keyboard.type(char, { delay: 45 });
      }
      await page.waitForTimeout(800);

      // Inject the user message + fake assistant response into localStorage
      await page.evaluate((pid) => {
        const key = `diraigent-chat-${pid}`;
        const msgs = JSON.parse(localStorage.getItem(key) || '[]');
        msgs.push({ role: 'user', content: 'Yes, create a subtask for the TLS investigation' });
        msgs.push({
          role: 'assistant',
          content: 'Done. Created task **#16 — Investigate Redis TLS certificate rotation** with the Research Spike playbook.\n\n' +
            'The task includes:\n' +
            '- Check TLS cert expiry and renewal configuration\n' +
            '- Test connection stability after cert rotation\n' +
            '- Document the rotation procedure\n\n' +
            'It\'s now in the ready queue and will be picked up by the next available agent.',
        });
        localStorage.setItem(key, JSON.stringify(msgs));
      }, PROJECT_ID);

      // Click send
      const sendBtn = page.locator('button:has-text("Send")');
      if (await sendBtn.isVisible({ timeout: 1000 }).catch(() => false)) {
        await sendBtn.click({ force: true });
      }
      await page.waitForTimeout(3000);
    } else {
      await page.waitForTimeout(2000);
    }

    // ── Scene 5: Playbooks ──────────────────────────────
    await page.locator('a:has-text("Playbooks")').click();
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);

    // Navigate to edit the first playbook directly
    await page.goto('/playbooks/pb-001/edit');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);

    // Scroll down to show steps
    await page.evaluate(() => window.scrollBy({ top: 400, behavior: 'smooth' }));
    await page.waitForTimeout(2500);

    // ── End ─────────────────────────────────────────────
    await page.waitForTimeout(500);
  });
});
