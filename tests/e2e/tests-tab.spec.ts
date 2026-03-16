import { test, expect } from '@playwright/test';

// Helper: navigate to the first project's detail page
async function goToFirstProject(page: any) {
  await page.goto('/#/projects');
  // Project tiles are divs with cursor-pointer class containing project names
  const tile = page.locator('.cursor-pointer h3').first();
  await expect(tile).toBeVisible({ timeout: 10000 });
  await tile.click();
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
}

// Helper: navigate to Tests tab on first project
async function goToTestsTab(page: any) {
  await goToFirstProject(page);
  const testsTab = page.locator('button', { hasText: /^Tests$/ });
  await expect(testsTab).toBeVisible({ timeout: 10000 });
  await testsTab.click();
  await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });
}

test.describe.serial('Tests tab and test runner', () => {
  test('Tests tab renders on project detail', async ({ page }) => {
    await goToFirstProject(page);

    // Verify the "Tests" tab button exists
    const testsTab = page.locator('button', { hasText: /^Tests$/ });
    await expect(testsTab).toBeVisible({ timeout: 10000 });
    await testsTab.click();

    // Verify the test run panel renders with run buttons
    await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });
    await expect(page.locator('button', { hasText: /^Unit$/ })).toBeVisible();
    await expect(page.locator('button', { hasText: /^Full$/ })).toBeVisible();
  });

  test('Run E2E button triggers a run', async ({ page }) => {
    await goToTestsTab(page);

    // Click "Run E2E" button
    const runBtn = page.locator('button', { hasText: /Run E2E/i });
    await runBtn.click();

    // Verify button becomes disabled (running state)
    await expect(runBtn).toBeDisabled({ timeout: 5000 });

    // The test will trigger a real Playwright run on the server which takes ~2min.
    // We just verify the button got disabled (run started) and a history entry appears.
    // Don't wait for full completion — that causes recursive test-within-test issues.
    // Instead, check that at least one run entry exists from previous runs.
    await expect(async () => {
      const entries = page.locator('button.w-full.text-left');
      const count = await entries.count();
      expect(count).toBeGreaterThanOrEqual(1);
    }).toPass({ timeout: 15000, intervals: [3000] });
  });

  test('Test run detail panel', async ({ page }) => {
    await goToTestsTab(page);

    // Click the first run entry in the history list (left panel)
    const historyEntry = page.locator('button.w-full.text-left').first();
    await expect(historyEntry).toBeVisible({ timeout: 10000 });
    await historyEntry.click();

    // Verify the detail panel shows status
    await expect(page.locator('text=/PASSED|FAILED|ERROR/')).toBeVisible({ timeout: 5000 });

    // Verify pass/fail counts are shown
    await expect(page.locator('text=/\\d+ passed/')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=/\\d+ failed/')).toBeVisible({ timeout: 5000 });

    // Verify "Show Full Output" toggle exists
    await expect(page.locator('button', { hasText: /Show.*Output/i })).toBeVisible({ timeout: 5000 });
  });

  test('Quick-trigger button exists on project tiles', async ({ page }) => {
    await page.goto('/#/projects');
    await expect(page.locator('.cursor-pointer h3').first()).toBeVisible({ timeout: 10000 });

    // Verify the play button (▶) exists on a project tile
    const playButton = page.locator('button[title="Run E2E tests"]').first();
    await expect(playButton).toBeVisible({ timeout: 10000 });

    // Verify it contains the play symbol
    const text = await playButton.textContent();
    expect(text?.trim()).toBe('▶');
  });

  test('Multiple test runs appear in history', async ({ page }) => {
    await goToTestsTab(page);

    // Verify the history list has more than one entry from previous test runs
    await expect(async () => {
      const count = await page.locator('button.w-full.text-left').count();
      expect(count).toBeGreaterThan(1);
    }).toPass({ timeout: 15000, intervals: [3000] });
  });
});
