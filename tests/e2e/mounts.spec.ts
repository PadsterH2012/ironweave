import { test, expect } from '@playwright/test';

test.describe('Mounts', () => {
  test('page loads at /#/mounts', async ({ page }) => {
    await page.goto('/#/mounts');

    const heading = page.locator('text=/mounts/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('shows mount list or empty state', async ({ page }) => {
    await page.goto('/#/mounts');

    // Either mount entries or an empty state message should be visible
    const content = page.locator('text=/mount|no mounts|empty/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('create mount button or form exists', async ({ page }) => {
    await page.goto('/#/mounts');

    const createButton = page.locator('button', { hasText: /create|add|new/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
  });
});
