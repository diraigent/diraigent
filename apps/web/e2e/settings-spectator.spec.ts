import { expect, test } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';
import { projects } from './fixtures/mock-data';

test.beforeEach(async ({ page }) => {
  await setupMocks(page);
});

test('sharing requires save, survives reload, preserves metadata, and can be disabled', async ({ page }) => {
  const unrelated = { custom: { nested: ['keep', 'these'] }, chat_model: 'custom/model', auto_push: true };
  let project = { ...projects[0], metadata: { ...unrelated } as Record<string, unknown> };
  const updates: Record<string, unknown>[] = [];
  await page.route(`${API}/${PROJECT_ID}`, route => {
    if (route.request().method() === 'PUT') {
      const update = route.request().postDataJSON();
      updates.push(update);
      project = { ...project, ...update };
    }
    return route.fulfill({ json: project });
  });
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: {
      writeText: async (text: string) => localStorage.setItem('copied-spectator-link', text),
    } });
  });

  await page.goto('/settings');
  const sharing = page.getByRole('checkbox', { name: 'Enable public read-only viewing' });
  const link = page.getByRole('textbox', { name: 'Spectator link', exact: true });
  const save = page.getByRole('button', { name: 'Save', exact: true }).first();
  await expect(sharing).not.toBeChecked();
  await expect(page.getByText(/Anyone with the link can read published task specs/)).toBeVisible();
  await expect(page.getByText(/Encrypted projects cannot be published/)).toBeVisible();
  await expect(link).toHaveCount(0);
  await sharing.check();
  expect(updates).toHaveLength(0);
  await expect(link).toHaveCount(0);
  await page.getByRole('textbox', { name: 'Name', exact: true }).fill('Shared project');
  await save.click();
  await expect(link).toHaveValue(`/spectate/${PROJECT_ID}`);
  expect(updates).toHaveLength(1);
  expect(updates[0]['name']).toBe('Shared project');
  expect(updates[0]['metadata']).toMatchObject({ ...unrelated, spectator_enabled: true });
  await expect(page.getByRole('link', { name: 'Open spectator view' })).toHaveAttribute('href', `/spectate/${PROJECT_ID}`);
  await page.getByRole('button', { name: 'Copy link', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Link copied');
  expect(await page.evaluate(() => localStorage.getItem('copied-spectator-link')))
    .toBe(new URL(`/spectate/${PROJECT_ID}`, page.url()).href);

  await page.reload();
  await expect(sharing).toBeChecked();
  await expect(page.getByRole('textbox', { name: 'Name', exact: true })).toHaveValue('Shared project');
  await expect(link).toBeVisible();
  await sharing.uncheck();
  // The active share link still reflects the server state until disable is saved.
  await expect(link).toBeVisible();
  await save.click();
  await expect(link).toHaveCount(0);
  expect(updates[1]['metadata']).toMatchObject({ ...unrelated, spectator_enabled: false });
  await page.reload();
  await expect(sharing).not.toBeChecked();
  await expect(link).toHaveCount(0);
});

for (const published of [false, true]) {
  test(`failed ${published ? 'disable' : 'enable'} reports an error without changing the saved share state`, async ({ page }) => {
    let project = { ...projects[0], metadata: { spectator_enabled: published } };
    let rejectSave = true;
    await page.route(`${API}/${PROJECT_ID}`, route => {
      if (route.request().method() === 'PUT') {
        if (rejectSave) return route.fulfill({ status: 400, json: { error: 'Publication rejected' } });
        project = { ...project, ...route.request().postDataJSON() };
      }
      return route.fulfill({ json: project });
    });
    await page.goto('/settings');
    const sharing = page.getByRole('checkbox', { name: 'Enable public read-only viewing' });
    await sharing.setChecked(!published);
    await page.getByRole('button', { name: 'Save', exact: true }).first().click();
    await expect(page.getByRole('alert')).toContainText('Your changes have not been saved');
    await expect(page.getByText('Saved', { exact: true })).toHaveCount(0);
    await expect(page.getByRole('textbox', { name: 'Spectator link', exact: true })).toHaveCount(published ? 1 : 0);
    await expect(sharing).toBeChecked({ checked: !published });
    await expect(page.getByRole('button', { name: 'Save', exact: true }).first()).toBeEnabled();
    rejectSave = false;
    await page.getByRole('button', { name: 'Save', exact: true }).first().click();
    await expect(page.getByText('Saved', { exact: true })).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
    await expect(page.getByRole('textbox', { name: 'Spectator link', exact: true })).toHaveCount(published ? 0 : 1);
  });
}

test('only boolean true is treated as published and copy errors are visible', async ({ page }) => {
  let enabled: unknown = 'true';
  await page.route(`${API}/${PROJECT_ID}`, route => route.fulfill({
    json: { ...projects[0], metadata: { spectator_enabled: enabled } },
  }));
  await page.addInitScript(() => {
    Object.defineProperty(navigator, 'clipboard', { value: {
      writeText: async () => { throw new Error('Clipboard unavailable'); },
    } });
  });
  await page.goto('/settings');
  await expect(page.getByRole('checkbox', { name: 'Enable public read-only viewing' })).not.toBeChecked();
  await expect(page.getByRole('textbox', { name: 'Spectator link', exact: true })).toHaveCount(0);
  enabled = true;
  await page.reload();
  await page.getByRole('button', { name: 'Copy link', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('Could not copy. Copy the link manually.');
});
