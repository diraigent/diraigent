import { expect, test } from '@playwright/test';
import { API, setupMocks } from './fixtures/setup';

test('authenticated viewer uses the normal app without agent chat', async ({ page }) => {
  await setupMocks(page);
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
});
