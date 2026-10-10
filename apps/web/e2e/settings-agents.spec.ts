import { expect, test } from '@playwright/test';
import { API, setupMocks } from './fixtures/setup';

test('agents remain visible when workspace administration is forbidden', async ({ page }) => {
  await setupMocks(page);
  for (const endpoint of ['roles', 'members']) {
    await page.route(`${API}/${endpoint}`, route => route.fulfill({
      status: 403, json: { error: 'Workspace administration required' },
    }));
  }
  await page.goto('/settings');
  await page.getByRole('button', { name: 'Agents', exact: true }).click();
  await expect(page.locator('app-settings table').getByText('claude-agent-1', { exact: true })).toBeVisible();
  await expect(page.locator('app-settings').getByText('An error occurred', { exact: true })).toHaveCount(0);
});

test('agent polling recovers after a failed request', async ({ page }) => {
  await page.clock.install();
  await setupMocks(page);
  let unavailable = true;
  await page.route(`${API}/agents`, async route => {
    if (unavailable) await route.fulfill({ status: 503, json: { error: 'Unavailable' } });
    else await route.fallback();
  });
  await page.goto('/settings');
  await page.getByRole('button', { name: 'Agents', exact: true }).click();
  await expect(page.locator('app-settings').getByText('An error occurred', { exact: true })).toBeVisible();
  unavailable = false;
  await page.clock.fastForward(11_000);
  await expect(page.locator('app-settings table').getByText('claude-agent-1', { exact: true })).toBeVisible();
  await expect(page.locator('app-settings').getByText('An error occurred', { exact: true })).toHaveCount(0);
});
