import { test, expect } from '@playwright/test';

test.describe('KillSwitch interactions', () => {
  test('global toggle on dashboard: pause then resume', async ({ page }) => {
    await page.goto('/#/');

    // Find the dispatch/killswitch section
    const section = page.locator('text=/dispatch|kill\\s*switch/i').first();
    await expect(section).toBeVisible({ timeout: 10000 });

    // Click pause button
    const pauseBtn = page.locator('button', { hasText: /pause/i }).first();
    await expect(pauseBtn).toBeVisible({ timeout: 5000 });
    await pauseBtn.click();

    // Verify status changes to Paused
    const pausedStatus = page.locator('text=/paused/i').first();
    await expect(pausedStatus).toBeVisible({ timeout: 5000 });

    // Resume to restore state
    const resumeBtn = page.locator('button', { hasText: /resume/i }).first();
    await expect(resumeBtn).toBeVisible({ timeout: 5000 });
    await resumeBtn.click();

    // Verify status goes back to active
    const activeStatus = page.locator('text=/active|running/i').first();
    await expect(activeStatus).toBeVisible({ timeout: 5000 });
  });

  test('per-project toggle: pause and resume on project tile', async ({ page }) => {
    await page.goto('/#/projects');

    // Find a project tile with a Pause button (Active project)
    const pauseBtn = page.locator('button', { hasText: /pause/i }).first();
    await expect(pauseBtn).toBeVisible({ timeout: 10000 });
    await pauseBtn.click();

    // Verify badge changes to Paused
    const pausedBadge = page.locator('text=/paused/i').first();
    await expect(pausedBadge).toBeVisible({ timeout: 5000 });

    // Resume to restore state
    const resumeBtn = page.locator('button', { hasText: /resume/i }).first();
    await expect(resumeBtn).toBeVisible({ timeout: 5000 });
    await resumeBtn.click();

    // Verify it goes back to active
    const activeBadge = page.locator('text=/active/i').first();
    await expect(activeBadge).toBeVisible({ timeout: 5000 });
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
