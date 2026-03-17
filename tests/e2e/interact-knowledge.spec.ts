import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();
const patternTitle = `E2E Test Pattern ${Date.now()}`;

async function goToProjectTab(page: any, tabName: string) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe.serial('Knowledge interactions on Ironweave project', () => {
  test('Create a pattern via API and verify in UI', async ({ page, request }) => {
    // Create via API (more reliable than form)
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/knowledge`, {
      data: {
        project_id: PROJECT_ID,
        pattern_type: 'solution',
        title: patternTitle,
        content: 'Test knowledge content',
        source_type: 'manual',
        keywords: ['test', 'e2e'],
      },
    });
    expect(res.ok()).toBeTruthy();

    // Navigate to Knowledge tab and verify pattern appears
    await goToProjectTab(page, 'Knowledge');
    await expect(async () => {
      const pattern = page.locator(`text=${patternTitle}`);
      await expect(pattern.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [3000] });
  });

  test('Pattern card shows correct info', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    // Wait for the created pattern to be visible
    await expect(async () => {
      const pattern = page.locator(`text=${patternTitle}`);
      await expect(pattern.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });

    // Verify type badge shows "solution"
    const card = page.locator('.rounded-xl', { hasText: patternTitle });
    const typeBadge = card.locator('text=/solution/i');
    await expect(typeBadge.first()).toBeVisible({ timeout: 5000 });

    // Verify confidence indicator exists (percentage text)
    const confidence = card.locator('text=/%/');
    await expect(confidence.first()).toBeVisible({ timeout: 5000 });
  });

  test('Delete pattern via API', async ({ request }) => {
    // Fetch all patterns to find the test one
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/knowledge`);
    expect(listRes.status()).toBe(200);
    const patterns = await listRes.json();
    const testPattern = patterns.find((p: any) => p.title === patternTitle);

    if (testPattern) {
      const delRes = await request.delete(
        `${BASE}/api/projects/${PROJECT_ID}/knowledge/${testPattern.id}`,
      );
      expect([200, 204]).toContain(delRes.status());
    }
  });

  test('Extract Now triggers extraction', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    const extractButton = page.locator('button', { hasText: /Extract Now/i });
    await expect(extractButton).toBeVisible({ timeout: 10000 });
    await extractButton.click();

    // Extraction may complete instantly (placeholder returns 0)
    // Just verify no error banner appeared and button is still functional
    await page.waitForTimeout(2000);
    const errorBanner = page.locator('.bg-red-500');
    const hasError = await errorBanner.count();
    expect(hasError).toBe(0);
  });
});
