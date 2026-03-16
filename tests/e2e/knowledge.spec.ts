import { test, expect } from '@playwright/test';

const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

async function goToProjectTab(page: any, tabName: string) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe('Knowledge tab — smoke tests', () => {
  test('Knowledge tab renders', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    // The knowledge panel content area should be visible
    const panel = page.locator('.space-y-4');
    await expect(panel.first()).toBeVisible({ timeout: 10000 });
  });

  test('Pattern list area renders', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    // Either pattern cards or the empty state message should be visible
    const patternArea = page.locator('.grid.grid-cols-1');
    await expect(patternArea.first()).toBeVisible({ timeout: 10000 });
  });

  test('Filter controls exist', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    // Type dropdown (select with "All Types" option)
    const typeDropdown = page.locator('select', { has: page.locator('option', { hasText: 'All Types' }) });
    await expect(typeDropdown.first()).toBeVisible({ timeout: 10000 });

    // Role filter input
    const roleInput = page.locator('input[placeholder="Filter by role..."]');
    await expect(roleInput).toBeVisible({ timeout: 10000 });
  });

  test('Add Pattern button exists', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    const addButton = page.locator('button', { hasText: 'Add Pattern' });
    await expect(addButton).toBeVisible({ timeout: 10000 });
  });

  test('Extract Now button exists', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    const extractButton = page.locator('button', { hasText: 'Extract Now' });
    await expect(extractButton).toBeVisible({ timeout: 10000 });
  });
});
