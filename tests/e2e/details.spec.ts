import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Details tab – smoke tests', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'networkidle', timeout: 15000 });
    const detailsTab = page.locator('button', { hasText: /^Details$/ });
    await expect(detailsTab).toBeVisible({ timeout: 10000 });
    await detailsTab.click();
  });

  test('Details tab renders', async ({ page }) => {
    // The details panel container should be visible (space-y-6 wrapper)
    await expect(page.locator('.space-y-6').first()).toBeVisible({ timeout: 10000 });
  });

  test('Intent panel exists', async ({ page }) => {
    // Intent heading
    await expect(page.getByText('Intent')).toBeVisible({ timeout: 10000 });
    // Textarea for intent content
    const textarea = page.locator('textarea[placeholder*="Describe what this project should be"]');
    await expect(textarea).toBeVisible({ timeout: 10000 });
  });

  test('Reality panel exists', async ({ page }) => {
    await expect(page.getByText('Reality')).toBeVisible({ timeout: 10000 });
    // Either the reality content pre element or the empty state message
    const realityContent = page.locator('pre').first();
    const emptyState = page.getByText('No reality scan yet. Click Rescan to generate.');
    await expect(realityContent.or(emptyState)).toBeVisible({ timeout: 10000 });
  });

  test('Save button exists', async ({ page }) => {
    const saveBtn = page.locator('button', { hasText: 'Save' });
    await expect(saveBtn.first()).toBeVisible({ timeout: 5000 });
  });

  test('Rescan button exists', async ({ page }) => {
    const rescanBtn = page.locator('button', { hasText: 'Rescan' });
    await expect(rescanBtn).toBeVisible({ timeout: 5000 });
  });
});
