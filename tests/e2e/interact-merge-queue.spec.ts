import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

async function goToMergeQueueTab(page: any) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: /^Merge Queue$/ });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe('Merge Queue on Ironweave project', () => {
  test('Merge Queue tab renders with heading', async ({ page }) => {
    await goToMergeQueueTab(page);
    await expect(page.locator('h2', { hasText: 'Merge Queue' })).toBeVisible({ timeout: 10000 });
  });

  test('Shows entries or empty state', async ({ page }) => {
    await goToMergeQueueTab(page);

    // Wait for content to load then check
    await expect(async () => {
      const emptyCount = await page.locator('text=No branches in merge queue').count();
      const entryCount = await page.locator('.font-mono').count();
      expect(emptyCount > 0 || entryCount > 0).toBeTruthy();
    }).toPass({ timeout: 10000, intervals: [2000] });
  });

  test('Merge queue API list returns array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(Array.isArray(data)).toBeTruthy();
  });

  test('Merge queue entry structure validation', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(Array.isArray(data)).toBeTruthy();

    for (const entry of data) {
      expect(entry).toHaveProperty('id');
      expect(entry).toHaveProperty('status');
      expect(entry).toHaveProperty('branch_name');
    }
  });
});
