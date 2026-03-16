/**
 * Playwright screenshot spec for branch review.
 *
 * Captures screenshots of all major pages and saves them to e2e/screenshots/
 * so they appear in branch diffs during code review.
 *
 * Usage:
 *   cd apps/web
 *   npm run e2e:screenshots
 */
import { test } from '@playwright/test';
import { setupMocks } from './fixtures/setup';

const DIR = 'e2e/screenshots';

test.describe('Branch review screenshots', () => {
  test.beforeEach(async ({ page }) => {
    await setupMocks(page);
  });

  test('dashboard', async ({ page }) => {
    await page.goto('/');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1500);
    await page.screenshot({ path: `${DIR}/dashboard.png`, fullPage: false });
  });

  test('work / goals', async ({ page }) => {
    await page.goto('/work');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/work.png`, fullPage: false });
  });

  test('review queue', async ({ page }) => {
    await page.goto('/review');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/review.png`, fullPage: false });
  });

  test('knowledge', async ({ page }) => {
    await page.goto('/knowledge');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/knowledge.png`, fullPage: false });
  });

  test('decisions', async ({ page }) => {
    await page.goto('/decisions');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/decisions.png`, fullPage: false });
  });

  test('playbooks', async ({ page }) => {
    await page.goto('/playbooks');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/playbooks.png`, fullPage: false });
  });

  test('playbook builder', async ({ page }) => {
    await page.goto('/playbooks/pb-001/edit');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/playbook-builder.png`, fullPage: false });
  });

  test('pipelines', async ({ page }) => {
    await page.goto('/pipelines');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/pipelines.png`, fullPage: false });
  });

  test('integrations', async ({ page }) => {
    await page.goto('/integrations');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/integrations.png`, fullPage: false });
  });

  test('verifications', async ({ page }) => {
    await page.goto('/verifications');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/verifications.png`, fullPage: false });
  });

  test('reports', async ({ page }) => {
    await page.goto('/reports');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/reports.png`, fullPage: false });
  });

  test('source', async ({ page }) => {
    await page.goto('/source');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/source.png`, fullPage: false });
  });

  test('audit', async ({ page }) => {
    await page.goto('/audit');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/audit.png`, fullPage: false });
  });

  test('settings', async ({ page }) => {
    await page.goto('/settings');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/settings.png`, fullPage: false });
  });

  test('tenant settings', async ({ page }) => {
    await page.goto('/tenant-settings');
    await page.waitForLoadState('networkidle');
    await page.waitForTimeout(1000);
    await page.screenshot({ path: `${DIR}/tenant-settings.png`, fullPage: false });
  });
});
