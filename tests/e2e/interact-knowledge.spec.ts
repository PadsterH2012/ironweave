import { test, expect } from '@playwright/test';

const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';
const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const patternTitle = `E2E Test Pattern ${Date.now()}`;

async function goToProjectTab(page: any, tabName: string) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe.serial('Knowledge interactions on Ironweave project', () => {
  test('Create a pattern via Add Pattern form', async ({ page }) => {
    await goToProjectTab(page, 'Knowledge');

    // Click "Add Pattern" to reveal the create form
    const addButton = page.locator('button', { hasText: 'Add Pattern' });
    await expect(addButton).toBeVisible({ timeout: 10000 });
    await addButton.click();

    // Fill title
    const titleInput = page.locator('input[placeholder="Title"]');
    await expect(titleInput).toBeVisible({ timeout: 5000 });
    await titleInput.fill(patternTitle);

    // Fill content
    const contentArea = page.locator('textarea[placeholder*="Content"]');
    await expect(contentArea).toBeVisible({ timeout: 5000 });
    await contentArea.fill('Test knowledge content');

    // Select type "solution" (already default, but explicitly set it)
    const typeSelect = page.locator('select').nth(1); // second select (first is filter)
    await typeSelect.selectOption('solution');

    // Click "Create"
    const createButton = page.locator('button', { hasText: /^Create$/ });
    await expect(createButton).toBeVisible({ timeout: 5000 });
    await createButton.click();

    // Verify the pattern appears in the list
    await expect(async () => {
      const pattern = page.locator(`text=${patternTitle}`);
      await expect(pattern.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
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

    const extractButton = page.locator('button', { hasText: 'Extract Now' });
    await expect(extractButton).toBeVisible({ timeout: 10000 });
    await extractButton.click();

    // Button should show "Extracting..." while in progress
    await expect(page.locator('button', { hasText: 'Extracting...' })).toBeVisible({ timeout: 5000 });

    // Wait for extraction to complete — button returns to "Extract Now"
    await expect(extractButton).toBeVisible({ timeout: 30000 });
  });
});
