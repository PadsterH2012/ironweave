import { test, expect } from '@playwright/test';

test.describe('KillSwitch', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/#/', { waitUntil: 'networkidle', timeout: 10000 });
  });

  test('killswitch renders on dashboard', async ({ page }) => {
    // The KillSwitch component should be visible on the dashboard
    const killswitch = page.locator('text=/dispatch|kill\\s*switch/i');
    await expect(killswitch.first()).toBeVisible({ timeout: 10000 });
  });

  test('shows pause/resume control', async ({ page }) => {
    // There should be a pause or resume button in the killswitch area
    const control = page.locator('button', { hasText: /pause|resume/i });
    await expect(control.first()).toBeVisible({ timeout: 10000 });
  });
});
