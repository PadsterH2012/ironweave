import { test, expect } from '@playwright/test';

test.describe('Dashboard', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/#/', { waitUntil: 'networkidle', timeout: 10000 });
  });

  test('stat cards are displayed', async ({ page }) => {
    const expectedLabels = ['Active Agents', 'In Progress', 'Open Issues', 'Closed'];
    for (const label of expectedLabels) {
      const card = page.locator('text=' + label);
      await expect(card).toBeVisible({ timeout: 10000 });
    }
  });

  test('KillSwitch component renders', async ({ page }) => {
    // The KillSwitch section should be present on the dashboard
    // It shows dispatch status with a pause/resume button
    const killswitch = page.locator('text=/dispatch|kill\\s*switch/i');
    await expect(killswitch.first()).toBeVisible({ timeout: 10000 });
  });

  test('system health panel renders', async ({ page }) => {
    // The SystemHealthPanel is rendered below the main content
    const healthPanel = page.locator('text=/system\\s*health|cpu|memory|disk/i');
    await expect(healthPanel.first()).toBeVisible({ timeout: 10000 });
  });
});
