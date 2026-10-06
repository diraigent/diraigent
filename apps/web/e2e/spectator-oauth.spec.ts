import { expect, test } from '@playwright/test';
import { API, PROJECT_ID } from './fixtures/setup';

test('OAuth remains required on normal routes, but never redirects public viewing', async ({ page }) => {
  const issuer = 'http://localhost:9000/application/o/diraigent/';
  const oauthRequests: string[] = [];
  await page.route('http://localhost:9000/**', route => {
    const url = route.request().url();
    oauthRequests.push(url);
    if (url.includes('.well-known')) return route.fulfill({ json: {
      issuer, authorization_endpoint: `${issuer}authorize`, token_endpoint: `${issuer}token`,
      userinfo_endpoint: `${issuer}userinfo`, jwks_uri: `${issuer}keys`,
    } });
    if (url.endsWith('/keys')) return route.fulfill({ json: { keys: [] } });
    return route.fulfill({ contentType: 'text/html', body: '<h1>OAuth sign in</h1>' });
  });
  await page.route(`${API}/**`, route => {
    const url = route.request().url().split('?')[0];
    if (url === `${API}/config`) return route.fulfill({ json: { auth_required: true } });
    if (url === `${API}/spectator/projects/${PROJECT_ID}`) return route.fulfill({ json: {
      id: PROJECT_ID, name: 'Public OAuth project', description: null, spectator: true,
    } });
    if (url === `${API}/spectator/projects/${PROJECT_ID}/tasks`) return route.fulfill({ json: {
      data: [], total: 0, limit: 20, offset: 0, has_more: false,
    } });
    return route.fulfill({ status: 401, json: { error: 'Sign in required' } });
  });
  await page.goto(`/spectate/${PROJECT_ID}`);
  await expect(page.getByRole('heading', { name: 'Public OAuth project' })).toBeVisible();
  expect(oauthRequests).toEqual([]);
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Public OAuth project' })).toBeVisible();
  expect(oauthRequests).toEqual([]);
  await page.getByRole('link', { name: 'Leave spectator view / Sign in' }).click();
  await expect(page.getByRole('heading', { name: 'OAuth sign in' })).toBeVisible();
  const url = new URL(page.url());
  expect(url.origin).toBe('http://localhost:9000');
  expect(url.pathname).toBe('/application/o/diraigent/authorize');
  expect(url.searchParams.get('response_type')).toBe('code');
  expect(url.searchParams.get('redirect_uri')).toBe('http://localhost:4200/auth/callback');
});
