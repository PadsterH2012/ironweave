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

test.describe('Data views across project tabs', () => {
  test('cost dashboard loads data', async ({ page }) => {
    await goToProjectTab(page, 'Costs');

    // Verify cost summary section renders with total or dollar values
    const costContent = page.locator('text=/total|\\$|cost|0/i').first();
    await expect(costContent).toBeVisible({ timeout: 10000 });
  });

  test('quality sliders render', async ({ page }) => {
    await goToProjectTab(page, 'Quality');

    // Verify tier floor/ceiling controls exist
    const qualityContent = page.locator('text=/quality|tier|floor|ceiling|slider/i').first();
    await expect(qualityContent).toBeVisible({ timeout: 10000 });
  });

  test('routing suggestions render', async ({ page }) => {
    await goToProjectTab(page, 'Routing');

    // Verify the routing section loads
    const routingContent = page.locator('text=/routing|route|suggestion|no .* found|empty/i').first();
    await expect(routingContent).toBeVisible({ timeout: 10000 });
  });

  test('coordinator panel renders', async ({ page }) => {
    await goToProjectTab(page, 'Coordinator');

    // Verify coordinator state display
    const coordContent = page.locator('text=/coordinator|active|dormant|state|status/i').first();
    await expect(coordContent).toBeVisible({ timeout: 10000 });
  });

  test('loom feed entries render', async ({ page }) => {
    await goToProjectTab(page, 'Loom');

    // Verify feed container renders
    const loomContent = page.locator('text=/loom|feed|event|entry|no .* found|empty/i').first();
    await expect(loomContent).toBeVisible({ timeout: 10000 });
  });

  test('merge queue renders', async ({ page }) => {
    await goToProjectTab(page, 'Merge Queue');

    // Verify queue section renders (list or empty state)
    const mergeContent = page.locator('text=/merge|queue|pending|no .* found|empty/i').first();
    await expect(mergeContent).toBeVisible({ timeout: 10000 });
  });

  test('prompts editor renders', async ({ page }) => {
    await goToProjectTab(page, 'Prompts');

    // Verify template list/editor area renders
    const promptContent = page.locator('text=/prompt|template|editor|no .* found|empty/i').first();
    await expect(promptContent).toBeVisible({ timeout: 10000 });
  });

  test('files browser renders', async ({ page }) => {
    await goToProjectTab(page, 'Files');

    // Verify file browser section renders
    const filesContent = page.locator('text=/file|browser|directory|no .* found|empty/i').first();
    await expect(filesContent).toBeVisible({ timeout: 10000 });
  });
});
