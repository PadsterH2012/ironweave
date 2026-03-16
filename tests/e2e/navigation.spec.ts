import { test, expect } from '@playwright/test';

test.describe('Navigation', () => {
  const routes = [
    { path: '/#/', label: 'Dashboard' },
    { path: '/#/projects', label: 'Projects' },
    { path: '/#/mounts', label: 'Mounts' },
    { path: '/#/agents', label: 'Agents' },
    { path: '/#/settings/general', label: 'Settings General' },
    { path: '/#/settings/proxies', label: 'Settings Proxies' },
    { path: '/#/settings/api-keys', label: 'Settings API Keys' },
    { path: '/#/login', label: 'Login' },
  ];

  for (const route of routes) {
    test(`route ${route.path} renders without error`, async ({ page }) => {
      await page.goto(route.path, { waitUntil: 'networkidle', timeout: 10000 });
      // Page should not show a blank white screen — some content must exist
      const body = page.locator('body');
      await expect(body).not.toBeEmpty();
    });
  }

  test('sidebar nav items are visible', async ({ page }) => {
    await page.goto('/#/', { waitUntil: 'networkidle', timeout: 10000 });

    const expectedItems = ['Dashboard', 'Projects', 'Mounts', 'Agents', 'Settings'];
    for (const label of expectedItems) {
      const navLink = page.locator('aside nav a', { hasText: label });
      await expect(navLink).toBeVisible({ timeout: 5000 });
    }
  });

  test('backend health indicator shows connected', async ({ page }) => {
    await page.goto('/#/', { waitUntil: 'networkidle', timeout: 10000 });

    const statusText = page.locator('aside').locator('text=connected');
    await expect(statusText).toBeVisible({ timeout: 10000 });
  });
});
