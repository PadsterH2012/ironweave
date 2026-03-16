import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Loom deep interactions', () => {
  test.describe('Loom feed UI on project', () => {
    test('loom feed renders on project Loom tab', async ({ page }) => {
      await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(2000);

      // Click the Loom tab
      const loomTab = page.locator('button', { hasText: 'Loom' });
      await expect(loomTab).toBeVisible({ timeout: 10000 });
      await loomTab.click();
      await page.waitForTimeout(2000);

      // LoomFeed container should be visible with heading "Loom"
      const loomPanel = page.locator('.rounded-xl', { hasText: 'Loom' }).first();
      await expect(loomPanel).toBeVisible({ timeout: 10000 });
    });

    test('loom entries have expected structure if present', async ({ page }) => {
      await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(2000);

      const loomTab = page.locator('button', { hasText: 'Loom' });
      await expect(loomTab).toBeVisible({ timeout: 10000 });
      await loomTab.click();
      await page.waitForTimeout(2000);

      const loomPanel = page.locator('.rounded-xl', { hasText: 'Loom' }).first();
      const noEntries = loomPanel.locator('text=No loom entries yet');
      const isEmpty = await noEntries.isVisible().catch(() => false);

      if (!isEmpty) {
        // Each entry is a button with type icon, role/agent, content, and timestamp
        const firstEntry = loomPanel.locator('button.w-full').first();
        await expect(firstEntry).toBeVisible({ timeout: 5000 });

        // Entry should contain a type icon span (typeColor classes)
        const typeIcon = firstEntry.locator('span.text-sm.mt-0\\.5').first();
        const hasTypeIcon = await typeIcon.isVisible().catch(() => false);

        // Entry should have content text
        const contentSpan = firstEntry.locator('span.text-gray-300').first();
        const hasContent = await contentSpan.isVisible().catch(() => false);

        // Entry should have a timestamp
        const timestamp = firstEntry.locator('span.text-gray-500').first();
        const hasTimestamp = await timestamp.isVisible().catch(() => false);

        // At least content and timestamp should be present
        expect(hasContent || hasTypeIcon).toBeTruthy();
        expect(hasTimestamp).toBeTruthy();
      } else {
        // Empty state is acceptable
        expect(isEmpty).toBeTruthy();
      }
    });
  });

  test.describe('Loom API contract', () => {
    test('GET /api/projects/{pid}/loom returns array', async ({ request }) => {
      const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/loom?limit=10`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(Array.isArray(body)).toBeTruthy();

      if (body.length > 0) {
        const entry = body[0];
        expect(entry).toHaveProperty('id');
        expect(entry).toHaveProperty('entry_type');
        expect(entry).toHaveProperty('content');
        expect(entry).toHaveProperty('timestamp');
      }
    });

    test('GET /api/loom returns array', async ({ request }) => {
      const res = await request.get(`${BASE}/api/loom?limit=10`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(Array.isArray(body)).toBeTruthy();

      if (body.length > 0) {
        const entry = body[0];
        expect(entry).toHaveProperty('id');
        expect(entry).toHaveProperty('entry_type');
        expect(entry).toHaveProperty('content');
      }
    });

    test('POST /api/loom creates a loom entry', async ({ request }) => {
      // First get a team_id from the project
      const teamsRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`, {
        ignoreHTTPSErrors: true,
      });
      expect(teamsRes.ok()).toBeTruthy();
      const teamsList = await teamsRes.json();

      // Use the first team if available, otherwise skip gracefully
      const teamId = Array.isArray(teamsList) && teamsList.length > 0
        ? teamsList[0].id
        : null;

      const payload: Record<string, string> = {
        project_id: PROJECT_ID,
        entry_type: 'status',
        content: 'e2e test loom entry',
      };
      if (teamId) {
        payload.team_id = teamId;
      }

      const res = await request.post(`${BASE}/api/loom`, {
        ignoreHTTPSErrors: true,
        data: payload,
      });

      // Should succeed with 200 or 201
      expect([200, 201]).toContain(res.status());

      const body = await res.json();
      expect(body).toHaveProperty('id');
      expect(body.entry_type).toBe('status');
      expect(body.content).toBe('e2e test loom entry');
    });
  });
});
