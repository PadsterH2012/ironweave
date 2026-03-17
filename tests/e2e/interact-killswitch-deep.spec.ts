import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

test.describe('KillSwitch deep tests', () => {
  test.describe('Dispatch status API contracts', () => {
    test('GET /api/dispatch/status returns paused and active_schedules', async ({ request }) => {
      const res = await request.get(`${BASE}/api/dispatch/status`);
      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(typeof body.paused).toBe('boolean');
      expect(Array.isArray(body.active_schedules)).toBeTruthy();
    });

    test('GET /api/projects/{pid}/dispatch/status returns paused and global_override', async ({ request }) => {
      const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/dispatch/status`);
      expect(res.ok()).toBeTruthy();
      const body = await res.json();
      expect(typeof body.paused).toBe('boolean');
      expect(typeof body.global_override).toBe('boolean');
    });
  });

  test.describe('Schedule API CRUD', () => {
    test('create, verify, and delete a dispatch schedule', async ({ request }) => {
      // Create (7-field cron: sec min hour dom month dow year)
      const createRes = await request.post(`${BASE}/api/dispatch/schedules`, {
        data: {
          scope: 'global',
          cron_expression: '0 30 17 * * 5 *',
          action: 'pause',
          timezone: 'Europe/London',
          description: 'e2e-api-test-schedule',
        },
      });
      expect(createRes.ok()).toBeTruthy();
      const created = await createRes.json();
      expect(created.id).toBeTruthy();
      expect(created.description).toBe('e2e-api-test-schedule');

      // Verify in list
      const listRes = await request.get(`${BASE}/api/dispatch/schedules`);
      expect(listRes.ok()).toBeTruthy();
      const schedules = await listRes.json();
      expect(schedules.find((s: any) => s.id === created.id)).toBeTruthy();

      // Delete
      const deleteRes = await request.delete(`${BASE}/api/dispatch/schedules/${created.id}`);
      expect(deleteRes.ok()).toBeTruthy();

      // Verify gone
      const listRes2 = await request.get(`${BASE}/api/dispatch/schedules`);
      const schedules2 = await listRes2.json();
      expect(schedules2.find((s: any) => s.id === created.id)).toBeFalsy();
    });
  });

  test.describe('Global pause/resume via API', () => {
    test('pause and resume global dispatch', async ({ request }) => {
      // Get initial state
      const initialRes = await request.get(`${BASE}/api/dispatch/status`);
      const initial = await initialRes.json();

      // Pause
      const pauseRes = await request.post(`${BASE}/api/dispatch/pause`, {
        data: { reason: 'e2e test' },
      });
      expect(pauseRes.ok()).toBeTruthy();
      const paused = await pauseRes.json();
      expect(paused.paused).toBe(true);

      // Resume
      const resumeRes = await request.post(`${BASE}/api/dispatch/resume`, { data: {} });
      expect(resumeRes.ok()).toBeTruthy();
      const resumed = await resumeRes.json();
      expect(resumed.paused).toBe(false);

      // Restore original state
      if (initial.paused) {
        await request.post(`${BASE}/api/dispatch/pause`, { data: {} });
      }
    });
  });

  test.describe('Per-project pause/resume via API', () => {
    test('pause and resume project dispatch', async ({ request }) => {
      // Get initial state
      const initialRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/dispatch/status`);
      const initial = await initialRes.json();

      // Pause project
      const pauseRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/dispatch/pause`, {
        data: { reason: 'e2e test' },
      });
      expect(pauseRes.ok()).toBeTruthy();
      const pausedProject = await pauseRes.json();
      expect(pausedProject.is_paused).toBe(true);

      // Resume project
      const resumeRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/dispatch/resume`, { data: {} });
      expect(resumeRes.ok()).toBeTruthy();
      const resumedProject = await resumeRes.json();
      expect(resumedProject.is_paused).toBe(false);

      // Restore original state
      if (initial.paused) {
        await request.post(`${BASE}/api/projects/${PROJECT_ID}/dispatch/pause`, { data: {} });
      }
    });
  });

  test.describe('Per-project toggle in UI', () => {
    test('project detail shows dispatch status badge', async ({ page }) => {
      await page.goto(`/#/projects/${PROJECT_ID}`);
      await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });

      // Should show one of: Active, Paused, or Global Pause Active
      const badge = page.locator('text=/Active|Paused|Global Pause/i');
      await expect(badge.first()).toBeVisible({ timeout: 10000 });
    });
  });
});
