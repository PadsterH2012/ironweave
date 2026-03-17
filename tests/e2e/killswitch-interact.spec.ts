import { test, expect } from '@playwright/test';
import { BASE } from './test-helpers';

test.describe('KillSwitch interactions', () => {
  test('global toggle on dashboard: toggle and restore', async ({ page, request }) => {
    // First check current state via API
    const statusRes = await request.get(`${BASE}/api/dispatch/status`);
    const status = await statusRes.json();
    const wasPaused = status.paused;

    await page.goto('/#/');
    await page.waitForTimeout(2000);

    if (wasPaused) {
      // Currently paused — resume first, then pause, then resume to restore
      const resumeBtn = page.locator('button', { hasText: /resume/i }).first();
      await expect(resumeBtn).toBeVisible({ timeout: 10000 });
      await resumeBtn.click();
      await page.waitForTimeout(2000);

      // Now pause
      const pauseBtn = page.locator('button', { hasText: /pause/i }).first();
      await expect(pauseBtn).toBeVisible({ timeout: 5000 });
      await pauseBtn.click();
      await page.waitForTimeout(1000);

      // Verify paused
      await expect(page.locator('text=/paused/i').first()).toBeVisible({ timeout: 5000 });

      // Restore original state (was paused)
      // Already paused, so we're good
    } else {
      // Currently active — pause, verify, then resume to restore
      const pauseBtn = page.locator('button', { hasText: /pause/i }).first();
      await expect(pauseBtn).toBeVisible({ timeout: 10000 });
      await pauseBtn.click();
      await page.waitForTimeout(1000);

      // Verify paused
      await expect(page.locator('text=/paused/i').first()).toBeVisible({ timeout: 5000 });

      // Resume to restore
      const resumeBtn = page.locator('button', { hasText: /resume/i }).first();
      await expect(resumeBtn).toBeVisible({ timeout: 5000 });
      await resumeBtn.click();
      await page.waitForTimeout(1000);
    }
  });

  test('per-project toggle: verify pause/resume buttons exist', async ({ page }) => {
    await page.goto('/#/projects');
    await page.waitForTimeout(2000);

    // Should have either Pause or Resume buttons on tiles
    const toggleBtn = page.locator('button', { hasText: /^(Pause|Resume)$/ }).first();
    await expect(toggleBtn).toBeVisible({ timeout: 10000 });
  });

  test('schedule visibility on dashboard', async ({ page }) => {
    await page.goto('/#/');

    // Find the killswitch section
    const section = page.locator('text=/dispatch|kill\\s*switch/i').first();
    await expect(section).toBeVisible({ timeout: 10000 });

    // Verify schedule list area exists (may be empty)
    const scheduleArea = page.locator('text=/schedule|cron|window/i').first();
    await expect(scheduleArea).toBeVisible({ timeout: 5000 });
  });
});
