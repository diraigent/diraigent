import {expect,test} from '@playwright/test';
import {API,PROJECT_ID,setupMocks} from './fixtures/setup';
import {projects} from './fixtures/mock-data';

test('workspace switching selects the workspace header and clears the previous project',async({page})=>{
  await setupMocks(page);
  const second='00000000-0000-0000-0000-000000000002';
  await page.route(`${API}/tenants`,r=>r.fulfill({json:[{id:'00000000-0000-0000-0000-000000000001',name:'Personal'},{id:second,name:'Shared'}]}));
  const headers:string[]=[];
  page.on('request',r=>{if(r.url().startsWith(API))headers.push(r.headers()['x-tenant-id']??'');});
  await page.goto('/work');
  await Promise.all([page.waitForEvent('load'),page.getByLabel('Workspace',{exact:true}).selectOption(second)]);
  await expect.poll(()=>page.evaluate(()=>localStorage.getItem('diraigent-workspace'))).toBe(second);
  await expect.poll(()=>headers.includes(second)).toBe(true);
  await expect(page).toHaveURL('/work');
});

test('project switching changes read-only access without logging out',async({page})=>{
  await setupMocks(page);
  const editor='b0000000-0000-4000-8000-000000000001';
  await page.route(API,r=>r.fulfill({json:[...projects,{...projects[0],id:editor,name:'Editable project'}]}));
  await page.route(`${API}/${PROJECT_ID}/people/me`,r=>r.fulfill({json:{role:'viewer',read_only:true}}));
  await page.route(`${API}/${editor}`,r=>r.fulfill({json:{...projects[0],id:editor,name:'Editable project'}}));
  await page.route(`${API}/${editor}/people/me`,r=>r.fulfill({json:{role:'editor',read_only:false}}));
  await page.goto('/work');
  await expect(page.getByRole('status').filter({hasText:'Read-only account'})).toBeVisible();
  await expect(page.locator('app-chat-drawer')).toHaveCount(0);
  await page.locator('app-project-switcher select').selectOption(editor);
  await expect(page.getByRole('status').filter({hasText:'Read-only account'})).toHaveCount(0);
  await expect(page.locator('app-chat-drawer')).toHaveCount(1);
});
