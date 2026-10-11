import { expect, test } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';

test('authenticated viewer uses the normal app without agent chat', async ({ page }) => {
  await setupMocks(page);
  const mutations: string[] = [];
  const deploymentLogReads: string[] = [];
  page.on('request', request => {
    if (!request.url().startsWith(API)) return;
    if (!['GET', 'HEAD', 'OPTIONS'].includes(request.method())) mutations.push(request.url());
    if (request.url().startsWith(`${API}/logs`)) deploymentLogReads.push(request.url());
  });
  await page.route(`${API}/${PROJECT_ID}/task-logs*`, route => route.fulfill({
    json: { data: [{ id: 'demo-log', step_name: 'Demo implementation' }], has_more: false },
  }));
  await page.route(`${API}/task-logs/demo-log`, route => route.fulfill({
    json: { id: 'demo-log', step_name: 'Demo implementation', content: 'Visible worker output [redacted]' },
  }));
  await page.route(`${API}/${PROJECT_ID}/audit*`, route => route.fulfill({
    json: { data: [{ id: 'demo-audit', action: 'task.updated', entity_type: 'task', summary: 'Visible audit [redacted]', created_at: '2026-01-01T12:00:00Z' }], total: 1, limit: 50, offset: 0, has_more: false },
  }));
  await page.route(`${API}/${PROJECT_ID}/source/blob*`, route => route.fulfill({
    json: { content: '// Visible source [redacted]', encoding: 'utf-8', size: 28 },
  }));
  await page.route(`${API}/account`, route => route.fulfill({
    json: { user_id: 'demo-user', read_only: true },
  }));
  await page.addInitScript(() => {
    sessionStorage.setItem('access_token', 'demo-test-token');
    sessionStorage.setItem('expires_at', String(Date.now() + 3600000));
    sessionStorage.setItem('access_token_stored_at', String(Date.now()));
    const originalFetch = window.fetch;
    window.fetch = (input, init) => {
      const url = typeof input === 'string' ? input : input instanceof URL ? input.href : input.url;
      if (url.endsWith('/config')) return Promise.resolve(new Response(JSON.stringify({
        auth_required: true, api_version: 'test', chat_model: 'test',
      }), { status: 200, headers: { 'Content-Type': 'application/json' } }));
      return originalFetch.call(window, input, init);
    };
  });
  await page.goto('/work');
  await expect(page.getByRole('status').filter({ hasText: 'Read-only account' })).toBeVisible();
  await expect(page.getByRole('complementary')).toBeVisible();
  await expect(page.locator('app-chat-drawer')).toHaveCount(0);
  await expect(page).toHaveURL('/work');
  await page.reload();
  await expect(page.getByRole('status').filter({ hasText: 'Read-only account' })).toBeVisible();
  await page.goto('/logs');
  await expect(page.getByRole('button', { name: 'Demo implementation' })).toBeVisible();
  await page.getByRole('button', { name: 'Demo implementation' }).click();
  await expect(page.getByText('Visible worker output [redacted]', { exact: true })).toBeVisible();
  await page.goto('/audit');
  await expect(page.getByText('Visible audit [redacted]', { exact: true })).toBeVisible();
  await page.goto('/source');
  await expect(page.getByText('README.md', { exact: true })).toBeVisible();
  await page.getByText('README.md', { exact: true }).click();
  await expect(page.getByText('// Visible source [redacted]', { exact: true })).toBeVisible();
  expect(mutations).toEqual([]);
  expect(deploymentLogReads).toEqual([]);
});
