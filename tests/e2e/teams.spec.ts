import { test, expect } from '@playwright/test';

async function goToProjectTab(page: any, tabName: string) {
  await page.goto('/#/projects');
  const tile = page.locator('.cursor-pointer h3').first();
  await expect(tile).toBeVisible({ timeout: 10000 });
  await tile.click();
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe('Teams tab', () => {
  test('Teams tab renders with team list or empty state', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    const content = page.locator('text=/team|no teams|empty/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Create Team button exists', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    const createButton = page.locator('button', { hasText: /new team/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
  });
});
