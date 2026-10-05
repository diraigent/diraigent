import { test, expect } from '@playwright/test';
import { setupMocks, API, PROJECT_ID } from './fixtures/setup';

test.use({ timezoneId: 'Europe/Stockholm' });
test.beforeEach(async ({ page }) => {
  page.on('pageerror', error => console.error('Browser error:', error.message));
  await page.route('**/config.js', route => route.fulfill({ contentType: 'application/javascript', body: '// Development config uses environment defaults.' }));
});

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

test('task screens expose direct execution without playbook requests', async ({ page }) => {
  await setupMocks(page);
  const retiredRequests: string[] = [];
  page.on('request', request => {
    if (/playbooks|step-templates/.test(request.url())) retiredRequests.push(request.url());
  });
  await page.goto('/work');
  await expect(page.locator('app-sidebar a[href="/playbooks"]')).toHaveCount(0);
  await page.getByText('Add user authentication flow', { exact: true }).first().click();
  await expect(page.locator('app-task-detail')).toBeVisible();
  await expect(page.locator('app-task-detail').getByText('Playbook', { exact: true })).toHaveCount(0);
  expect(retiredRequests).toEqual([]);
});

test('project setup proceeds directly to agent assignment', async ({ page }) => {
  await setupMocks(page);
  await page.route(API, route => route.request().method() === 'POST'
    ? route.fulfill({json: {id:PROJECT_ID, name:'Direct project', slug:'direct', default_branch:'main', metadata:{}}})
    : route.fallback());
  await page.goto('/work');
  await page.getByTitle('New Project', {exact:true}).click();
  const modal = page.locator('app-create-project-modal');
  await modal.getByPlaceholder('Project name', {exact:true}).fill('Direct project');
  await modal.getByRole('button', {name:'Next',exact:true}).click();
  await expect(modal.getByRole('heading', {name:'Assign Agent',exact:true})).toBeVisible();
  await expect(modal.getByText('Select Default Playbook', {exact:true})).toHaveCount(0);
  await modal.getByRole('button', {name:'Skip',exact:true}).click();
  await expect(modal.getByText('Direct project', {exact:true})).toBeVisible();
});
