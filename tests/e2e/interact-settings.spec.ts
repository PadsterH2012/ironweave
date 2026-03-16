import { test, expect } from '@playwright/test';

test.describe('Settings interactions', () => {
  test('general settings page shows form fields', async ({ page }) => {
    await page.goto('/#/settings/general');

    // Settings page uses labels + inputs, not a table
    // Verify the key form fields exist
    await expect(page.locator('label', { hasText: /Browse Roots/i })).toBeVisible({ timeout: 10000 });
    await expect(page.locator('label', { hasText: /Mount Base/i })).toBeVisible({ timeout: 10000 });
    await expect(page.locator('button', { hasText: /Save/i })).toBeVisible({ timeout: 5000 });
  });

  test('navigate to Proxies tab and verify it loads', async ({ page }) => {
    await page.goto('/#/settings/proxies');

    // Verify proxies content loads
    const proxiesContent = page.locator('text=/prox/i').first();
    await expect(proxiesContent).toBeVisible({ timeout: 10000 });
  });

  test('navigate to API Keys tab and verify it loads', async ({ page }) => {
    await page.goto('/#/settings/api-keys');

    // Verify API keys content loads
    const apiKeysContent = page.locator('text=/api/i').first();
    await expect(apiKeysContent).toBeVisible({ timeout: 10000 });
  });
});
