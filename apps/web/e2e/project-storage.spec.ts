import { expect, test } from '@playwright/test';
import { API, PROJECT_ID, setupMocks } from './fixtures/setup';

const worker = '11111111-2222-3333-4444-555555555555';
test.beforeEach(async ({ page }) => {
  await setupMocks(page);
  await page.addInitScript(() => localStorage.setItem('diraigent-chat-collapsed', 'false'));
});

test('loads personal history, sends its revision, and avoids browser content storage', async ({ page }) => {
  let messages = [{role:'user',content:'Prior question'}, {role:'assistant',content:'Saved on Orchestra'}];
  let revision = 2;
  await page.route(`${API}/${PROJECT_ID}/chat/history`, route => route.fulfill({json:{enabled:true,revision,messages,busy:false}}));
  let sent: Record<string, unknown> | undefined;
  await page.route(`${API}/${PROJECT_ID}/chat`, route => {
    sent = route.request().postDataJSON();
    messages = [...messages, {role:'user',content:'New question'}, {role:'assistant',content:'Persisted response'}];
    revision = 4;
    return route.fulfill({contentType:'text/event-stream',body:'event: done\ndata: {"message":{"role":"assistant","content":"Persisted response"}}\n\n'});
  });
  await page.goto('/work');
  await expect(page.getByText('Saved on Orchestra', {exact:true})).toBeVisible();
  await page.getByPlaceholder('Ask about your project...').fill('New question');
  await page.getByPlaceholder('Ask about your project...').press('Enter');
  await expect.poll(() => sent?.['history_revision']).toBe(2);
  await expect(page.getByText('Persisted response', {exact:true})).toBeVisible();
  expect(await page.evaluate(() => Object.values(localStorage).some(value => value.includes('Persisted response')))).toBe(false);
  await page.reload();
  await expect(page.getByText('Persisted response', {exact:true})).toBeVisible();
});

test('offline content blocks sending and recovers after owner reconnects', async ({ page }) => {
  let offline = true;
  await page.route(`${API}/${PROJECT_ID}/chat/history`, route => offline
    ? route.fulfill({status:503,json:{error:'Storage owner offline'}})
    : route.fulfill({json:{enabled:true,revision:0,messages:[],busy:false}}));
  await page.goto('/work');
  await expect(page.getByPlaceholder('Ask about your project...')).toBeDisabled();
  await expect(page.getByText('No project selected', {exact:true})).toHaveCount(0);
  offline = false;
  await page.getByRole('button', {name:'Reload conversation',exact:true}).click();
  await expect(page.getByPlaceholder('Ask about your project...')).toBeEnabled();
});

test('manager assigns an updated owner and runs bounded migration', async ({ page }) => {
  let owner: string | null = null;
  let migrated = false;
  await page.route(`${API}/agents`, route => route.fulfill({json:[{id:worker,name:'Project Orchestra',status:'idle',metadata:{content_protocol:1}}]}));
  await page.route(`${API}/${PROJECT_ID}/storage`, route => {
    if (route.request().method() === 'PUT') owner = route.request().postDataJSON().agent_id;
    return route.fulfill({json:{agent_id:owner,content_protocol:1}});
  });
  await page.route(`${API}/${PROJECT_ID}/storage/migrate`, route => {
    const moved = migrated ? 0 : 3; migrated = true;
    return route.fulfill({json:{moved,batch_limit_per_kind:20}});
  });
  await page.goto('/settings');
  await page.getByLabel('Storage owner', {exact:true}).selectOption(worker);
  await page.getByRole('button', {name:'Use Orchestra storage',exact:true}).click();
  await expect(page.getByText('Storage owner: Project Orchestra', {exact:true})).toBeVisible();
  await page.getByRole('button', {name:'Move existing content to Orchestra',exact:true}).click();
  await expect(page.getByRole('status').filter({hasText:'Migration complete. Moved 3 records'})).toBeVisible();
});
