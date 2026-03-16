import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test('stat cards are displayed', async ({ page }) => {
    await page.goto('/#/');

    const expectedLabels = ['Active Agents', 'In Progress', 'Open Issues', 'Closed'];
    for (const label of expectedLabels) {
      await expect(page.locator(`text=${label}`)).toBeVisible({ timeout: 10000 });
    }
  });

  test('KillSwitch component renders', async ({ page }) => {
    await page.goto('/#/');

    const dispatch = page.locator('text=Dispatch');
    await expect(dispatch.first()).toBeVisible({ timeout: 10000 });
  });

  test('system health panel renders', async ({ page }) => {
    await page.goto('/#/');

    const healthIndicator = page.locator('text=/CPU|Memory/i');
    await expect(healthIndicator.first()).toBeVisible({ timeout: 10000 });
  });
});
