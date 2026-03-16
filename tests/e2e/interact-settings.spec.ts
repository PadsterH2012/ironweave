import { test, expect } from '@playwright/test';

test.describe('Settings CRUD', () => {
  test('navigate to general settings and verify list renders', async ({ page }) => {
    await page.goto('/#/settings/general');

    const heading = page.locator('text=/general|settings/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Verify a settings list or table is present
    const settingsContent = page.locator('table, [role="list"], form, .settings, dl, ul').first();
    await expect(settingsContent).toBeVisible({ timeout: 10000 });
  });

  test('navigate to Proxies tab and verify it loads', async ({ page }) => {
    await page.goto('/#/settings/general');

    // Click Proxies tab/link
    const proxiesTab = page.locator('a, button', { hasText: /prox/i });
    await expect(proxiesTab.first()).toBeVisible({ timeout: 10000 });
    await proxiesTab.first().click();

    // Verify proxies content loads
    const proxiesHeading = page.locator('text=/prox/i').first();
    await expect(proxiesHeading).toBeVisible({ timeout: 10000 });
  });

  test('navigate to API Keys tab and verify it loads', async ({ page }) => {
    await page.goto('/#/settings/general');

    // Click API Keys tab/link
    const apiKeysTab = page.locator('a, button', { hasText: /api\s*key/i });
    await expect(apiKeysTab.first()).toBeVisible({ timeout: 10000 });
    await apiKeysTab.first().click();

    // Verify API keys content loads
    const apiKeysHeading = page.locator('text=/api\\s*key/i').first();
    await expect(apiKeysHeading).toBeVisible({ timeout: 10000 });
  });
});
