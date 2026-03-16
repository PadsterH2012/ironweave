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

test.describe('Workflows tab', () => {
  test('workflow section renders', async ({ page }) => {
    await goToProjectTab(page, 'Workflows');

    const content = page.locator('text=/workflow|no workflows|empty/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });
});
