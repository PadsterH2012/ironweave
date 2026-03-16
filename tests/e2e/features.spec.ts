import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Features tab – smoke tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'networkidle', timeout: 15000 });
    const featuresTab = page.locator('button', { hasText: /^Features$/ });
    await expect(featuresTab).toBeVisible({ timeout: 10000 });
    await featuresTab.click();
  });

  test('Features tab renders', async ({ page }) => {
    // The feature panel container should be visible (space-y-4 wrapper)
    await expect(page.locator('.space-y-4').first()).toBeVisible({ timeout: 10000 });
  });

  test('Feature list or empty state visible', async ({ page }) => {
    // Either feature cards or the empty state message should be visible
    const featureCard = page.locator('.rounded-xl.bg-gray-900.border.border-gray-800');
    const emptyState = page.getByText('No features yet. Add one or import a PRD.');
    await expect(featureCard.first().or(emptyState)).toBeVisible({ timeout: 10000 });
  });

  test('Status filter buttons exist', async ({ page }) => {
    const filters = ['All', 'Ideas', 'Designed', 'In Progress', 'Implemented', 'Verified', 'Parked'];
    for (const label of filters) {
      await expect(page.locator('button', { hasText: label }).first()).toBeVisible({ timeout: 5000 });
    }
  });

  test('Add Feature button exists', async ({ page }) => {
    const addBtn = page.locator('button', { hasText: 'Add Feature' });
    await expect(addBtn).toBeVisible({ timeout: 5000 });
  });

  test('Import PRD button exists', async ({ page }) => {
    const importBtn = page.locator('button', { hasText: 'Import PRD' });
    await expect(importBtn).toBeVisible({ timeout: 5000 });
  });
});
