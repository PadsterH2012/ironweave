import { test, expect } from '@playwright/test';

test.describe('Settings', () => {
  test('general settings page loads', async ({ page }) => {
    await page.goto('/#/settings/general', { waitUntil: 'networkidle', timeout: 10000 });

    const heading = page.locator('text=/general|settings/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('proxies settings page loads', async ({ page }) => {
    await page.goto('/#/settings/proxies', { waitUntil: 'networkidle', timeout: 10000 });

    const heading = page.locator('text=/prox/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('API keys settings page loads', async ({ page }) => {
    await page.goto('/#/settings/api-keys', { waitUntil: 'networkidle', timeout: 10000 });

    const heading = page.locator('text=/api\\s*key/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });
});
