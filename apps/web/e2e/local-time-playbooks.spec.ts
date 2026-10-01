import { test, expect } from '@playwright/test';
import { setupMocks, PROJECT_ID } from './fixtures/setup';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

test.use({ timezoneId: 'Europe/Stockholm' });

test('task activity uses local time including daylight saving', async ({ page }) => {
  await setupMocks(page);
  await page.route('**/verifications?*', route => route.fulfill({ json: {data: [], total: 0} }));
  await page.route('**/tasks/*/related', route => route.fulfill({ json: {knowledge: [], decisions: [], observations: []} }));
  await page.route('**/tasks/*/dependencies', route => route.fulfill({ json: {depends_on: [], blocks: []} }));
  const dates = ['2026-10-01T18:43:27Z', '2026-01-01T18:43:27Z'];
  await page.route('**/tasks/*/updates', route => route.fulfill({ json: dates.map((created_at, i) => ({
    id: `u${i}`, kind: 'progress', content: 'Local time regression', metadata: {}, created_at,
  })) }));
  await page.route('**/tasks/*/comments', route => route.fulfill({ json: dates.map((created_at, i) => ({
    id: `c${i}`, content: 'Local comment time', created_at, agent_id: null,
  })) }));
  await page.goto('/work');
  await page.getByText('Add user authentication flow', { exact: true }).first().click();
  await expect(page.locator('app-task-updates time')).toHaveText(['20:43', '19:43'], { timeout: 15000 });
  await expect(page.locator('app-task-comments time')).toHaveText(['20:43', '19:43'], { timeout: 15000 });
});

test('bundled playbooks load through the project endpoint', async ({ page }) => {
  await setupMocks(page);
  const books = ['standard-lifecycle', 'standard-backlog-start', 'dreamer', 'researcher'].map(name => ({
    ...JSON.parse(readFileSync(join(process.cwd(), '../../libs/common-rust/diraigent-types/resources/playbooks', `${name}.json`), 'utf8')),
    tenant_id: null,
  }));
  await page.route(`**/projects/${PROJECT_ID}/playbooks`, route => route.fulfill({ json: books }));
  await page.goto('/playbooks');
  for (const book of books) await expect(page.locator('span').filter({ hasText: new RegExp(`^${book.title.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')}$`) }).first()).toBeVisible();
});
