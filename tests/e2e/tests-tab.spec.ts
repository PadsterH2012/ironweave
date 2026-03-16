import { test, expect } from '@playwright/test';

test.describe.serial('Tests tab and test runner', () => {
  test('Tests tab renders on project detail', async ({ page }) => {
    await page.goto('/#/projects');
    await page.waitForSelector('[data-testid="project-tile"], a[href*="projects/"]', { timeout: 10000 });

    // Click the first project tile/link
    const firstProject = page.locator('[data-testid="project-tile"], a[href*="projects/"]').first();
    await firstProject.click();
    await page.waitForURL(/\/#\/projects\/.+/);

    // Verify the "Tests" tab button exists and click it
    const testsTab = page.locator('button', { hasText: /^Tests$/i });
    await expect(testsTab).toBeVisible({ timeout: 10000 });
    await testsTab.click();

    // Verify the test run panel renders with run buttons
    await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });
    await expect(page.locator('button', { hasText: /Unit/i })).toBeVisible();
    await expect(page.locator('button', { hasText: /Full/i })).toBeVisible();
  });

  test('Trigger a test run from the Tests tab', async ({ page }) => {
    // Navigate to the first project's Tests tab
    await page.goto('/#/projects');
    await page.waitForSelector('[data-testid="project-tile"], a[href*="projects/"]', { timeout: 10000 });
    await page.locator('[data-testid="project-tile"], a[href*="projects/"]').first().click();
    await page.waitForURL(/\/#\/projects\/.+/);
    await page.locator('button', { hasText: /^Tests$/i }).click();
    await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });

    // Click "Run E2E" button
    await page.locator('button', { hasText: /Run E2E/i }).click();

    // Verify button shows running state (disabled or text change)
    await expect(async () => {
      const btn = page.locator('button', { hasText: /Running|Run E2E/i }).first();
      const isDisabled = await btn.isDisabled();
      const text = await btn.textContent();
      expect(isDisabled || /running/i.test(text || '')).toBeTruthy();
    }).toPass({ timeout: 5000 });

    // Wait for the run to complete — the panel auto-refreshes every 3s
    // Look for a history entry with a status (passed or failed)
    await expect(async () => {
      const statusIndicator = page.locator('text=/passed|failed/i');
      await expect(statusIndicator.first()).toBeVisible();
    }).toPass({ timeout: 30000, intervals: [3000] });

    // Verify a test run entry appears in the history list
    const historyEntries = page.locator('[data-testid="test-run-entry"], [data-testid="run-history"] > *, li:has-text("passed"), li:has-text("failed"), tr:has-text("passed"), tr:has-text("failed")');
    await expect(historyEntries.first()).toBeVisible({ timeout: 5000 });
  });

  test('Test run detail panel', async ({ page }) => {
    // Navigate to the first project's Tests tab
    await page.goto('/#/projects');
    await page.waitForSelector('[data-testid="project-tile"], a[href*="projects/"]', { timeout: 10000 });
    await page.locator('[data-testid="project-tile"], a[href*="projects/"]').first().click();
    await page.waitForURL(/\/#\/projects\/.+/);
    await page.locator('button', { hasText: /^Tests$/i }).click();
    await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });

    // Wait for at least one history entry, then click it
    const historyEntry = page.locator('[data-testid="test-run-entry"], [data-testid="run-history"] > *, li:has-text("passed"), li:has-text("failed"), tr:has-text("passed"), tr:has-text("failed")').first();
    await expect(historyEntry).toBeVisible({ timeout: 10000 });
    await historyEntry.click();

    // Verify the detail panel shows status, counts, and duration
    await expect(page.locator('text=/passed|failed/i').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=/pass|fail|skip/i').first()).toBeVisible();
    await expect(page.locator('text=/\\d+\\.?\\d*\\s*s|duration/i').first()).toBeVisible({ timeout: 5000 });

    // Verify "Show Full Output" toggle exists
    const outputToggle = page.locator('button, label, [role="switch"]', { hasText: /Show Full Output/i });
    await expect(outputToggle).toBeVisible({ timeout: 5000 });
  });

  test('Quick-trigger button on project tiles', async ({ page }) => {
    await page.goto('/#/projects');
    await page.waitForSelector('[data-testid="project-tile"], a[href*="projects/"]', { timeout: 10000 });

    // Find the play button on a project tile
    const playButton = page.locator('button:has-text("▶"), button[aria-label*="run" i], button[aria-label*="test" i], button[title*="run" i]').first();
    await expect(playButton).toBeVisible({ timeout: 10000 });
    await playButton.click();

    // Verify it shows a running indicator
    await expect(async () => {
      const runningIndicator = page.locator('text=/⟳/').or(page.locator('.animate-pulse')).or(page.locator('[data-testid="running-indicator"]'));
      await expect(runningIndicator.first()).toBeVisible();
    }).toPass({ timeout: 5000 });

    // Wait for completion
    await expect(async () => {
      const resultIndicator = page.locator('text=/✓|✗|passed|failed/').first();
      await expect(resultIndicator).toBeVisible();
    }).toPass({ timeout: 30000, intervals: [3000] });
  });

  test('Multiple test runs appear in history', async ({ page }) => {
    // Navigate to the first project's Tests tab
    await page.goto('/#/projects');
    await page.waitForSelector('[data-testid="project-tile"], a[href*="projects/"]', { timeout: 10000 });
    await page.locator('[data-testid="project-tile"], a[href*="projects/"]').first().click();
    await page.waitForURL(/\/#\/projects\/.+/);
    await page.locator('button', { hasText: /^Tests$/i }).click();
    await expect(page.locator('button', { hasText: /Run E2E/i })).toBeVisible({ timeout: 10000 });

    // Verify the history list has more than one entry from previous test runs
    const historyEntries = page.locator('[data-testid="test-run-entry"], [data-testid="run-history"] > *, li:has-text("passed"), li:has-text("failed"), tr:has-text("passed"), tr:has-text("failed")');
    await expect(async () => {
      const count = await historyEntries.count();
      expect(count).toBeGreaterThan(1);
    }).toPass({ timeout: 15000, intervals: [3000] });
  });
});
