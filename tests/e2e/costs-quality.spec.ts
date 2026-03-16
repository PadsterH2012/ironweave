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

test.describe('Costs, Quality, Routing, and Coordinator tabs', () => {
  test('Costs tab renders cost dashboard', async ({ page }) => {
    await goToProjectTab(page, 'Costs');

    const content = page.locator('text=/cost|spend|budget|total|\\\$/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Quality tab renders quality slider', async ({ page }) => {
    await goToProjectTab(page, 'Quality');

    const content = page.locator('text=/quality|slider|score|threshold/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Routing tab renders routing suggestions', async ({ page }) => {
    await goToProjectTab(page, 'Routing');

    const content = page.locator('text=/routing|route|suggestion|model/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Coordinator tab renders coordinator panel', async ({ page }) => {
    await goToProjectTab(page, 'Coordinator');

    const content = page.locator('text=/coordinator|orchestrat|dispatch/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });
});
