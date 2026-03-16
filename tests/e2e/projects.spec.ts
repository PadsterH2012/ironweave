import { test, expect } from '@playwright/test';

test.describe('Projects', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/#/projects', { waitUntil: 'networkidle', timeout: 10000 });
  });

  test('project list page renders', async ({ page }) => {
    // The page heading should be visible
    const heading = page.locator('text=/projects/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('create project form exists', async ({ page }) => {
    // There should be a button or control to create a new project
    const createButton = page.locator('button', { hasText: /new project|create/i });
    await expect(createButton).toBeVisible({ timeout: 10000 });
  });
});
