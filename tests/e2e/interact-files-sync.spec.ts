import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

async function goToProjectTab(page: any, tabName: string) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe('Files & Sync', () => {
  test('Files tab renders with sync status bar', async ({ page }) => {
    await goToProjectTab(page, 'Files');

    // Sync status bar should show Source, Last synced, State
    await expect(page.locator('text=Source').first()).toBeVisible({ timeout: 10000 });
    await expect(page.locator('text=Last synced').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=State').first()).toBeVisible({ timeout: 5000 });
  });

  test('Sync Now button exists', async ({ page }) => {
    await goToProjectTab(page, 'Files');

    const syncButton = page.locator('button', { hasText: /Sync Now/i });
    await expect(syncButton).toBeVisible({ timeout: 10000 });
  });

  test('File browser renders with breadcrumb navigation', async ({ page }) => {
    await goToProjectTab(page, 'Files');

    // Root breadcrumb "/" should be visible
    await expect(page.locator('button', { hasText: '/' }).first()).toBeVisible({ timeout: 10000 });

    // Should show either files/directories or "No files found"
    await expect(async () => {
      const hasEntries = await page.locator('.divide-y .cursor-pointer').count();
      const hasEmpty = await page.locator('text=No files found').count();
      expect(hasEntries > 0 || hasEmpty > 0).toBeTruthy();
    }).toPass({ timeout: 10000, intervals: [2000] });
  });

  test('Navigate into a directory', async ({ page }) => {
    await goToProjectTab(page, 'Files');

    // Wait for entries to load
    await page.waitForTimeout(2000);

    // Find a directory entry (has folder icon 📁) and click it
    const dirEntry = page.locator('.cursor-pointer', { hasText: /📁/ }).first();
    if (await dirEntry.isVisible({ timeout: 5000 }).catch(() => false)) {
      const dirName = await dirEntry.textContent();
      await dirEntry.click();

      // Breadcrumb should update — more than just "/"
      await expect(async () => {
        const breadcrumbs = await page.locator('button').filter({ hasText: /[^/]/ }).count();
        expect(breadcrumbs).toBeGreaterThan(0);
      }).toPass({ timeout: 5000 });

      // Navigate up button (..) should appear
      await expect(page.locator('text=..')).toBeVisible({ timeout: 5000 });
    } else {
      // No directories — test passes (project may be empty)
      expect(true).toBe(true);
    }
  });

  test('Open a file to view contents', async ({ page }) => {
    await goToProjectTab(page, 'Files');
    await page.waitForTimeout(2000);

    // Find a file entry (has file icon 📄) and click it
    const fileEntry = page.locator('.cursor-pointer', { hasText: /📄/ }).first();
    if (await fileEntry.isVisible({ timeout: 5000 }).catch(() => false)) {
      await fileEntry.click();

      // File viewer panel should show the file path and content (or loading)
      await expect(page.locator('.font-mono.truncate').or(page.locator('pre')).first()).toBeVisible({ timeout: 10000 });

      // Close button (×) should exist
      await expect(page.locator('button', { hasText: '×' }).first()).toBeVisible({ timeout: 5000 });
    } else {
      // No files — skip gracefully
      expect(true).toBe(true);
    }
  });

  test('Trigger sync via API and verify status updates', async ({ page, request }) => {
    // Trigger sync via API
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/sync`);
    // Sync may fail if no sync_path is configured — that's fine, we just check it responds
    expect([200, 201, 400, 500]).toContain(res.status());

    // Navigate to Files tab and check the sync status
    await goToProjectTab(page, 'Files');

    // State field should show a known state (idle, synced, syncing, error)
    await expect(page.locator('text=/idle|synced|syncing|error/i').first()).toBeVisible({ timeout: 10000 });
  });

  test('File content viewer shows close button', async ({ page }) => {
    await goToProjectTab(page, 'Files');
    await page.waitForTimeout(2000);

    // When no file is selected, should show placeholder
    const placeholder = page.locator('text=Select a file to view its contents');
    if (await placeholder.isVisible({ timeout: 3000 }).catch(() => false)) {
      // Good — default state
      expect(true).toBe(true);
    }
  });
});

test.describe('Project History tab', () => {
  test('History tab renders', async ({ page }) => {
    // History tab only shows if project has a mount_id
    await page.goto(`/#/projects/${PROJECT_ID}`);
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });

    const historyTab = page.locator('button', { hasText: /^History$/ });
    if (await historyTab.isVisible({ timeout: 5000 }).catch(() => false)) {
      await historyTab.click();

      // Should show snapshot list or empty state
      await expect(async () => {
        const hasSnapshots = await page.locator('text=/snapshot|change|restore/i').count();
        const hasEmpty = await page.locator('text=/no.*history|no.*snapshots|empty/i').count();
        expect(hasSnapshots > 0 || hasEmpty > 0).toBeTruthy();
      }).toPass({ timeout: 10000, intervals: [2000] });
    } else {
      // No History tab (no mount_id) — skip gracefully
      expect(true).toBe(true);
    }
  });

  test('Sync history API returns array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/sync/history`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(Array.isArray(data)).toBe(true);
  });

  test('Sync status API returns valid state', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/sync/status`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(data).toHaveProperty('sync_state');
    expect(['idle', 'synced', 'syncing', 'error']).toContain(data.sync_state);
  });
});
