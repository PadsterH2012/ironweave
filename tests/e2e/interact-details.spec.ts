import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

async function goToDetailsTab(page: any) {
  await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'networkidle', timeout: 15000 });
  const detailsTab = page.locator('button', { hasText: /^Details$/ });
  await expect(detailsTab).toBeVisible({ timeout: 10000 });
  await detailsTab.click();
  await expect(page.locator('.space-y-6').first()).toBeVisible({ timeout: 10000 });
}

test.describe('Details tab – interaction tests', () => {
  test('save intent content via API and verify in UI', async ({ page, request }) => {
    const testContent = `Test intent ${Date.now()}`;

    // Save intent via API
    const res = await request.put(`${BASE}/api/projects/${PROJECT_ID}/documents/intent`, {
      data: { content: testContent },
    });
    expect(res.status()).toBeLessThan(300);

    // Navigate to Details tab and verify content appears
    await goToDetailsTab(page);
    const textarea = page.locator('textarea[placeholder*="Describe what this project should be"]');
    await expect(textarea).toBeVisible({ timeout: 10000 });
    await expect(textarea).toHaveValue(testContent, { timeout: 10000 });
  });

  test('gap analysis section renders', async ({ page, request }) => {
    // Fetch gaps to confirm endpoint works
    const gapsRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/documents/gaps`);
    expect(gapsRes.status()).toBe(200);

    // Navigate to Details tab and check for gap analysis section
    await goToDetailsTab(page);

    // The gap analysis section may or may not appear depending on data.
    // Check that the page renders without errors (Intent and Reality are visible).
    await expect(page.getByText('Intent')).toBeVisible({ timeout: 10000 });
    await expect(page.getByText('Reality')).toBeVisible({ timeout: 10000 });

    // If gaps exist, the Gap Analysis heading should be visible
    const gapsData = await gapsRes.json();
    const hasGaps =
      (gapsData.missing && gapsData.missing.length > 0) ||
      (gapsData.undocumented && gapsData.undocumented.length > 0);

    if (hasGaps) {
      await expect(page.getByText('Gap Analysis')).toBeVisible({ timeout: 10000 });
    }
  });
});
