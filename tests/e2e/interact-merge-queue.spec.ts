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
  test('Merge Queue tab renders', async ({ page }) => {
    await goToMergeQueueTab(page);

    // The component renders an h2 "Merge Queue" heading
    const heading = page.locator('h2', { hasText: 'Merge Queue' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Either entries exist or the empty state is shown
    const content = page.locator('text=No branches in merge queue').or(
      page.locator('.font-mono').first()
    );
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Merge queue API list', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();
    expect(Array.isArray(data)).toBeTruthy();
  });

  test('Queue entries display', async ({ page }) => {
    await goToMergeQueueTab(page);

    const heading = page.locator('h2', { hasText: 'Merge Queue' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Check for empty state or entry content
    const emptyState = page.locator('text=No branches in merge queue');
    const entryCard = page.locator('.font-mono').first();

    // Wait for either to appear
    await expect(emptyState.or(entryCard)).toBeVisible({ timeout: 10000 });

    const isEmpty = await emptyState.isVisible();
    if (!isEmpty) {
      // Entries present: verify branch name (font-mono) and status badge (rounded-full) are shown
      await expect(page.locator('.font-mono').first()).toBeVisible();
      await expect(page.locator('.rounded-full').first()).toBeVisible();
    } else {
      await expect(emptyState).toHaveText('No branches in merge queue');
    }
  });

  test('Merge queue entry structure', async ({ request }) => {
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
