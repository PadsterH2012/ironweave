import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('KillSwitch deep interactions', () => {
  test.describe('Schedule CRUD via UI', () => {
    test('create a schedule, verify it appears, then delete it', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(2000);

      // Expand the schedules section by clicking "Show Schedules"
      const showBtn = page.locator('button', { hasText: /show schedules/i }).first();
      await expect(showBtn).toBeVisible({ timeout: 10000 });
      await showBtn.click();
      await page.waitForTimeout(500);

      // The "Add Schedule" form should now be visible
      await expect(page.locator('text=Add Schedule')).toBeVisible({ timeout: 5000 });

      // Fill in the cron expression
      const cronInput = page.locator('input[placeholder*="Cron"]');
      await expect(cronInput).toBeVisible({ timeout: 5000 });
      await cronInput.fill('0 9 * * 1-5');

      // Select "pause" action (should be default, but be explicit)
      const actionSelect = page.locator('select').filter({ has: page.locator('option[value="pause"]') }).first();
      await actionSelect.selectOption('pause');

      // Fill in a description so we can identify it
      const descInput = page.locator('input[placeholder*="Description"]');
      await descInput.fill('e2e-test-schedule');

      // Click Add
      const addBtn = page.locator('button', { hasText: /^Add$/ });
      await expect(addBtn).toBeEnabled();
      await addBtn.click();
      await page.waitForTimeout(2000);

      // Verify the schedule appears in the list
      await expect(page.locator('text=0 9 * * 1-5')).toBeVisible({ timeout: 5000 });
      await expect(page.locator('text=e2e-test-schedule')).toBeVisible({ timeout: 5000 });

      // Delete the schedule by clicking the x button next to it
      const scheduleRow = page.locator('.rounded-lg', { hasText: '0 9 * * 1-5' }).filter({ hasText: 'e2e-test-schedule' });
      const deleteBtn = scheduleRow.locator('button[title="Delete schedule"]');
      await deleteBtn.click();
      await page.waitForTimeout(1500);

      // Verify the schedule is gone
      await expect(page.locator('text=e2e-test-schedule')).not.toBeVisible({ timeout: 5000 });
    });
  });

  test.describe('Per-project pause from project detail', () => {
    test('toggle dispatch pause/resume on Ironweave project', async ({ page, request }) => {
      // Get current project dispatch status
      const statusRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/dispatch/status`, {
        ignoreHTTPSErrors: true,
      });
      const initialStatus = await statusRes.json();
      const wasPaused = initialStatus.paused;

      await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(2000);

      if (wasPaused) {
        // Currently paused - click Resume, verify Active badge, then Pause to restore
        const resumeBtn = page.locator('button', { hasText: /^Resume$/ }).first();
        await expect(resumeBtn).toBeVisible({ timeout: 10000 });
        await resumeBtn.click();
        await page.waitForTimeout(2000);

        // Verify Active badge appears
        await expect(page.locator('text=Active').first()).toBeVisible({ timeout: 5000 });

        // Restore: click Pause
        const pauseBtn = page.locator('button', { hasText: /^Pause$/ }).first();
        await expect(pauseBtn).toBeVisible({ timeout: 5000 });
        await pauseBtn.click();
        await page.waitForTimeout(1500);

        // Verify Paused badge
        await expect(page.locator('text=Paused').first()).toBeVisible({ timeout: 5000 });
      } else {
        // Currently active - click Pause, verify Paused badge, then Resume to restore
        const pauseBtn = page.locator('button', { hasText: /^Pause$/ }).first();
        await expect(pauseBtn).toBeVisible({ timeout: 10000 });
        await pauseBtn.click();
        await page.waitForTimeout(2000);

        // Verify Paused badge appears
        await expect(page.locator('text=Paused').first()).toBeVisible({ timeout: 5000 });

        // Restore: click Resume
        const resumeBtn = page.locator('button', { hasText: /^Resume$/ }).first();
        await expect(resumeBtn).toBeVisible({ timeout: 5000 });
        await resumeBtn.click();
        await page.waitForTimeout(1500);

        // Verify Active badge
        await expect(page.locator('text=Active').first()).toBeVisible({ timeout: 5000 });
      }
    });
  });

  test.describe('Dispatch status API contract', () => {
    test('GET /api/dispatch/status returns paused boolean and active_schedules array', async ({ request }) => {
      const res = await request.get(`${BASE}/api/dispatch/status`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(typeof body.paused).toBe('boolean');
      expect(Array.isArray(body.active_schedules)).toBeTruthy();
    });

    test('GET /api/projects/{pid}/dispatch/status returns paused and global_override', async ({ request }) => {
      const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/dispatch/status`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(typeof body.paused).toBe('boolean');
      expect(typeof body.global_override).toBe('boolean');
    });
  });

  test.describe('Schedule API CRUD', () => {
    let createdScheduleId: string | null = null;

    test('POST, GET, and DELETE a dispatch schedule', async ({ request }) => {
      // POST: create a schedule
      const createRes = await request.post(`${BASE}/api/dispatch/schedules`, {
        ignoreHTTPSErrors: true,
        data: {
          scope: 'global',
          cron_expression: '30 17 * * FRI',
          action: 'pause',
          timezone: 'Europe/London',
          description: 'e2e-api-test-schedule',
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const created = await createRes.json();
      expect(created.id).toBeTruthy();
      createdScheduleId = created.id;

      // GET: verify it appears in the list
      const listRes = await request.get(`${BASE}/api/dispatch/schedules`, {
        ignoreHTTPSErrors: true,
      });
      expect(listRes.ok()).toBeTruthy();
      const schedules = await listRes.json();
      expect(Array.isArray(schedules)).toBeTruthy();
      const found = schedules.find((s: any) => s.id === createdScheduleId);
      expect(found).toBeTruthy();
      expect(found.cron_expression).toBe('30 17 * * FRI');
      expect(found.description).toBe('e2e-api-test-schedule');

      // DELETE: remove the schedule
      const deleteRes = await request.delete(`${BASE}/api/dispatch/schedules/${createdScheduleId}`, {
        ignoreHTTPSErrors: true,
      });
      expect(deleteRes.ok()).toBeTruthy();

      // Verify it's gone
      const listRes2 = await request.get(`${BASE}/api/dispatch/schedules`, {
        ignoreHTTPSErrors: true,
      });
      const schedules2 = await listRes2.json();
      const notFound = schedules2.find((s: any) => s.id === createdScheduleId);
      expect(notFound).toBeFalsy();
    });
  });
});
