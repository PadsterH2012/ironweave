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

test.describe('Issues tab', () => {
  test('issue board renders with column headers', async ({ page }) => {
    await goToProjectTab(page, 'Issues');

    const columns = ['Open', 'In Progress', 'Review', 'Closed'];
    for (const col of columns) {
      await expect(page.locator(`text=${col}`).first()).toBeVisible({ timeout: 10000 });
    }
  });
});
