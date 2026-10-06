import { expect, test, Page } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';

const root = `${API}/spectator/projects/${PROJECT_ID}`;
const link = `/spectate/${PROJECT_ID}`;
const hostile = '<img src=x onerror="window.spectatorXss=true">';
const task = {
  id: 'task-1', number: 1, title: 'Published task', kind: 'feature', state: 'ready',
  created_at: '2026-10-01T00:00:00Z', updated_at: '2026-10-01T00:00:00Z', completed_at: null,
  spec: hostile, acceptance_criteria: ['Published criterion'],
};
const items = {
  tasks: task,
  work: { id: 'work-1', title: 'Published work', status: 'active', work_type: 'feature',
    description: 'Public description', success_criteria: ['Work criterion'] },
  knowledge: { id: 'knowledge-1', title: 'Published knowledge', category: 'architecture',
    content: 'Public knowledge content', tags: ['Public tag'] },
  decisions: { id: 'decision-1', title: 'Published decision', status: 'accepted',
    context: 'Public context', decision: 'Public choice', rationale: 'Public rationale' },
};

async function publicMocks(page: Page, savedToken = false) {
  const requests: { url: string; method: string; headers: Record<string, string> }[] = [];
  await page.addInitScript(({ savedToken }) => {
    localStorage.setItem('diraigent-project', 'private-project-selection');
    if (savedToken) {
      sessionStorage.setItem('access_token', 'existing-member-token');
      sessionStorage.setItem('expires_at', String(Date.now() + 3600000));
    }
  }, { savedToken });
  await page.route('http://localhost:9000/**', route => route.abort());
  await page.route(`${API}/**`, async route => {
    const request = route.request();
    requests.push({ url: request.url(), method: request.method(), headers: request.headers() });
    const url = new URL(request.url());
    const path = url.pathname.replace('/v1/spectator/projects/', '');
    if (request.url().split('?')[0] === root) {
      return route.fulfill({ json: { id: PROJECT_ID, name: 'Published project', description: 'Overview', spectator: true } });
    }
    const [, area, id] = path.split('/');
    const item = items[area as keyof typeof items];
    if (item) {
      if (id) return route.fulfill({ json: item });
      const offset = Number(url.searchParams.get('offset'));
      const data = area === 'tasks' && offset > 0 ? [{ ...task, id: 'task-2', title: 'Second page task' }] : [item];
      return route.fulfill({ json: { data, total: 21, limit: 20, offset,
        has_more: area === 'tasks' && (offset === 0 || offset === 1_000_000) } });
    }
    return route.fulfill({ status: 401, json: { error: 'Private endpoint must not be called' } });
  });
  return requests;
}

for (const savedToken of [false, true]) {
  test(`public entry and refresh are isolated (${savedToken ? 'member token' : 'anonymous OAuth visitor'})`, async ({ page }) => {
    const requests = await publicMocks(page, savedToken);
    await page.goto(link);
    await expect(page.getByRole('heading', { name: 'Published project' })).toBeVisible();
    // Exercise shell priority with an actually logged-in AuthService, not just
    // stored tokens. Public startup itself deliberately does not initialize it.
    const authState = await page.evaluate(savedToken => {
      const ng = (window as unknown as { ng: {
        getComponent(element: Element): { auth: {
          markInitialized(): void; isLoggedIn(): boolean; isAuthDisabled(): boolean;
        } };
        applyChanges(component: unknown): void;
      } }).ng;
      const app = ng.getComponent(document.querySelector('app-root')!);
      if (savedToken) app.auth.markInitialized();
      ng.applyChanges(app);
      return { loggedIn: app.auth.isLoggedIn(), disabled: app.auth.isAuthDisabled() };
    }, savedToken);
    expect(authState).toEqual({ loggedIn: savedToken, disabled: false });
    await expect(page.getByText('Spectator · Read-only')).toBeVisible();
    await expect(page.locator('app-sidebar, app-chat-drawer, app-create-project-modal')).toHaveCount(0);
    for (const key of ['c', 'n', 'e', '?', '1']) await page.keyboard.press(key);
    await expect(page.locator('app-keyboard-help')).toHaveCount(0);
    await expect(page).toHaveURL(link);
    expect(await page.evaluate(() => localStorage.getItem('diraigent-project'))).toBe('private-project-selection');
    await page.reload();
    await expect(page.getByRole('heading', { name: 'Published project' })).toBeVisible();
    expect(await page.evaluate(() => localStorage.getItem('diraigent-project'))).toBe('private-project-selection');
    expect(requests.length).toBeGreaterThan(0);
    for (const request of requests) {
      expect(request.url).toContain('/v1/spectator/projects/');
      expect(request.method).toBe('GET');
      expect(request.headers['authorization']).toBeUndefined();
      expect(request.headers['x-dev-user-id']).toBeUndefined();
    }
  });
}

test('bounded paging, all public detail views, safe text and refresh deep links', async ({ page }) => {
  const requests = await publicMocks(page);
  await page.goto(link);
  await page.getByRole('link', { name: 'Next', exact: true }).click();
  await expect(page.getByRole('link', { name: 'Second page task' })).toBeVisible();
  expect(requests.some(r => r.url.includes('limit=20&offset=20'))).toBe(true);
  await page.getByRole('link', { name: 'Previous', exact: true }).click();
  await page.getByRole('link', { name: 'Published task', exact: true }).click();
  await expect(page.getByText(hostile, { exact: true })).toBeVisible();
  await expect(page.locator('app-spectator img')).toHaveCount(0);
  await expect(page.getByText('Published criterion')).toBeVisible();
  await page.reload();
  await expect(page.getByText(hostile, { exact: true })).toBeVisible();
  await page.getByRole('link', { name: 'Back to list' }).click();
  for (const [area, label, content] of [
    ['work', 'Work', 'Work criterion'],
    ['knowledge', 'Knowledge', 'Public knowledge content'],
    ['decisions', 'Decisions', 'Public rationale'],
  ]) {
    await page.getByRole('link', { name: label, exact: true }).click();
    await page.getByRole('link', { name: items[area as keyof typeof items].title, exact: true }).click();
    await expect(page.getByText(content, { exact: true })).toBeVisible();
  }
  await page.goto(`${link}?offset=-20`);
  await expect(page.getByRole('link', { name: 'Published task' })).toBeVisible();
  expect(requests.at(-1)!.url).not.toContain('offset=-');
  await page.goto(`${link}?area=source&offset=1000000000`);
  await expect(page.getByRole('link', { name: 'Second page task' })).toBeVisible();
  expect(requests.some(r => r.url.includes('/tasks?limit=20&offset=1000000'))).toBe(true);
  await expect(page.getByRole('link', { name: 'Next', exact: true })).toHaveCount(0);
  expect(requests.some(r => r.url.includes('/source'))).toBe(false);
});

test('revocation on a subsequent request clears the entire project, list and detail', async ({ page }) => {
  await publicMocks(page);
  await page.goto(`${link}?area=tasks&id=task-1`);
  await expect(page.getByText('Published criterion')).toBeVisible();
  await page.route(`${root}/knowledge*`, route => route.fulfill({ status: 404, json: { error: 'unavailable' } }));
  await page.getByRole('link', { name: 'Knowledge', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Project unavailable');
  await expect(page.getByText('Published project', { exact: true })).toHaveCount(0);
  await expect(page.getByText('Published criterion')).toHaveCount(0);
  await expect(page.getByRole('link', { name: 'Published task' })).toHaveCount(0);
});

test('missing project, loading and empty states do not expose stale content', async ({ page }) => {
  await publicMocks(page);
  await page.route(root, async route => {
    await new Promise(resolve => setTimeout(resolve, 500));
    await route.fulfill({ status: 404, json: { error: 'private name must not be rendered' } });
  });
  await page.goto(link);
  await expect(page.getByRole('status')).toContainText('Loading');
  await expect(page.getByRole('alert')).toContainText('Project unavailable');
  await expect(page.getByText('private name must not be rendered')).toHaveCount(0);
  await page.unroute(root);
  await page.route(`${root}/tasks*`, route => route.fulfill({ json: { data: [], total: 0, limit: 20, offset: 0, has_more: false } }));
  await page.getByRole('button', { name: 'Try again' }).click();
  await expect(page.getByRole('status')).toContainText('No published items.');
});

test('explicit exit restores normal authenticated startup and shell', async ({ page }) => {
  await setupMocks(page);
  await page.route(`${API}/dashboard/summary*`, route => route.fulfill({ json: { projects: [], tokens_per_day: [] } }));
  await page.route(root, route => route.fulfill({ json: {
    id: PROJECT_ID, name: 'Published project', description: null, spectator: true,
  } }));
  await page.route(`${root}/tasks*`, route => route.fulfill({ json: {
    data: [], total: 0, limit: 20, offset: 0, has_more: false,
  } }));
  await page.goto(link);
  await expect(page.getByText('Spectator · Read-only')).toBeVisible();
  await page.getByRole('link', { name: 'Leave spectator view / Sign in' }).click();
  await expect(page).toHaveURL('/dashboard');
  await expect(page.locator('app-sidebar aside')).toBeVisible();
  await expect(page.locator('#chat-panel')).toBeVisible();
});
