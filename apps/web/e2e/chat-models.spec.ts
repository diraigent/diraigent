import { expect, test } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';
import { projects } from './fixtures/mock-data';

const agent = '11111111-2222-3333-4444-555555555555';

test.beforeEach(async ({ page }) => {
  await setupMocks(page);
  await page.addInitScript(() => localStorage.setItem('diraigent-chat-collapsed', 'false'));
});

test('discovers, searches and sends a selected model to its worker', async ({ page }) => {
  await page.route(`${API}/${PROJECT_ID}/chat/models*`, route => route.fulfill({ json: {
    provider: 'opencode', default_model: 'custom/default', models: ['custom/default', 'custom/fast'], agent_id: agent,
  } }));
  let requestBody: Record<string, unknown> | undefined;
  await page.route(`${API}/${PROJECT_ID}/chat`, route => {
    requestBody = route.request().postDataJSON();
    return route.fulfill({ contentType: 'text/event-stream', body: 'event: done\ndata: {"message":{"role":"assistant","content":"Done"}}\n\n' });
  });
  await page.goto('/work');
  const picker = page.getByRole('button', { name: 'Choose chat model' });
  await expect(picker).toContainText('custom/default');
  await picker.click();
  await page.getByRole('textbox', { name: 'Search models' }).fill('fast');
  await expect(page.getByRole('button', { name: 'custom/default', exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'custom/fast', exact: true }).click();
  await expect(picker).toContainText('custom/fast');
  await page.getByPlaceholder('Ask about your project...').fill('Hello');
  await page.getByPlaceholder('Ask about your project...').press('Enter');
  await expect.poll(() => requestBody?.['model']).toBe('custom/fast');
  expect(requestBody?.['agent_id']).toBe(agent);
});

test('manual entry remains usable when discovery fails and refresh retries', async ({ page }) => {
  let requests = 0;
  await page.route(`${API}/${PROJECT_ID}/chat/models*`, route => {
    requests++;
    return route.fulfill({ status: 503, json: { error: 'Worker unavailable' } });
  });
  await page.goto('/work');
  const picker = page.getByRole('button', { name: 'Choose chat model' });
  await picker.click();
  await expect(page.getByText('Model list unavailable. Refresh or enter a model manually.')).toBeVisible();
  const before = requests;
  await page.getByRole('button', { name: 'Refresh', exact: true }).click();
  await expect.poll(() => requests).toBeGreaterThan(before);
  await page.getByLabel('Custom model', { exact: true }).fill('custom/manual');
  await page.getByRole('button', { name: 'Use model', exact: true }).click();
  await expect(picker).toContainText('custom/manual');
});

test('switching projects clears the previous model and catalog', async ({ page }) => {
  const second = { ...projects[0], id: '22222222-2222-4222-8222-222222222222', name: 'Second project' };
  await page.route(new RegExp(`^${API}/?$`), route => route.fulfill({ json: [...projects, second] }));
  await page.route(`${API}/${second.id}`, route => route.fulfill({ json: second }));
  await page.route(`${API}/${second.id}/**`, route => route.fallback({
    url: route.request().url().replace(second.id, PROJECT_ID),
  }));
  await page.route(`${API}/${PROJECT_ID}/chat/models*`, route => route.fulfill({ json: {
    provider: 'opencode', default_model: null, models: ['custom/first-project'], agent_id: agent,
  } }));
  await page.route(`${API}/${second.id}/chat/models*`, route => route.fulfill({ json: {
    provider: 'opencode', default_model: null, models: ['custom/second-project'], agent_id: agent,
  } }));
  await page.goto('/work');
  const picker = page.getByRole('button', { name: 'Choose chat model' });
  await picker.click();
  await page.getByRole('button', { name: 'custom/first-project', exact: true }).click();
  await page.locator('app-project-switcher select').selectOption(second.id);
  await expect(picker).toContainText('Worker default');
  await picker.click();
  await expect(page.getByRole('button', { name: 'custom/second-project', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'custom/first-project', exact: true })).toHaveCount(0);
});
