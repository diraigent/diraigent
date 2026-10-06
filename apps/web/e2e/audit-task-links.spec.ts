import { expect, test } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';
import { tasks } from './fixtures/mock-data';

const task = { ...tasks.data[0], id: 'a0000000-0000-4000-8000-000000000012', title: 'Task opened from audit' };
const entry = {
  id: 'audit-task-link', project_id: PROJECT_ID, entity_type: 'task', entity_id: task.id,
  action: 'updated', summary: 'Updated task from audit', actor_name: null,
  actor_agent_id: null, actor_user_id: null, before_state: null, after_state: null,
  metadata: {}, created_at: '2026-10-06T06:00:00Z',
};

test.beforeEach(async ({ page }) => {
  await setupMocks(page);
  await page.route(`${API}/${PROJECT_ID}/audit`, route => route.fulfill({
    json: { data: [entry], total: 1, limit: 50, offset: 0, has_more: false },
  }));
  await page.route(`${API}/tasks/${task.id}`, route => route.fulfill({ json: task }));
});

test('audit task ID opens the actual task even when absent from the Work list', async ({ page }) => {
  await page.goto('/audit');
  await page.getByRole('button', { name: /Updated task from audit/ }).click();
  const link = page.getByRole('link', { name: task.id, exact: true });
  await expect(link).toHaveAttribute('href', `/work?taskId=${task.id}`);
  await link.focus();
  await page.keyboard.press('Enter');
  await expect(page).toHaveURL(`/work?taskId=${task.id}`);
  const detail = page.getByTestId('deep-linked-task');
  await expect(detail).toContainText(task.title);
  await expect(detail.locator('app-task-detail')).toBeVisible();
  await page.reload();
  await expect(detail.locator('app-task-detail')).toBeVisible();
});

for (const variant of [{ entity_type: 'work', action: 'updated' }, { entity_type: 'task', action: 'deleted' }]) {
  test(`${variant.entity_type} ${variant.action} audit ID remains plain text`, async ({ page }) => {
    await page.route(`${API}/${PROJECT_ID}/audit`, route => route.fulfill({
      json: { data: [{ ...entry, ...variant }], total: 1, limit: 50, offset: 0, has_more: false },
    }));
    await page.goto('/audit');
    await page.getByRole('button', { name: /Updated task from audit/ }).click();
    await expect(page.getByText(task.id, { exact: false })).toBeVisible();
    await expect(page.getByRole('link', { name: task.id })).toHaveCount(0);
  });
}

test('unavailable task deep link reports an error', async ({ page }) => {
  await page.route(`${API}/tasks/${task.id}`, route => route.fulfill({ status: 404, json: { error: 'Not found' } }));
  await page.goto(`/work?taskId=${task.id}`);
  await expect(page.getByRole('alert')).toContainText('An error occurred');
  await expect(page.getByTestId('deep-linked-task')).toHaveCount(0);
});

for (const state of ['backlog', 'done', 'cancelled']) {
  test(`${state} task deep link opens without expanding filtered sections`, async ({ page }) => {
    await page.route(`${API}/tasks/${task.id}`, route => route.fulfill({ json: { ...task, state } }));
    await page.goto(`/work?taskId=${task.id}`);
    await expect(page.getByTestId('deep-linked-task').locator('app-task-detail')).toBeVisible();
  });
}
