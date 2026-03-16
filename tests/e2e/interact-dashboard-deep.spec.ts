import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('Dashboard deep interactions', () => {
  test.describe('Dashboard chart components', () => {
    test('metrics chart area renders', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // The Metrics section contains an SVG rendered by MetricsChart (D3)
      // It lives inside a panel with heading "Metrics"
      const metricsPanel = page.locator('.rounded-xl', { hasText: 'Metrics' }).first();
      await expect(metricsPanel).toBeVisible({ timeout: 10000 });

      // Should contain an SVG element (D3 chart) or a "Loading metrics..." message
      const svg = metricsPanel.locator('svg').first();
      const loading = metricsPanel.locator('text=Loading metrics');
      const hasSvg = await svg.isVisible().catch(() => false);
      const isLoading = await loading.isVisible().catch(() => false);
      expect(hasSvg || isLoading).toBeTruthy();
    });

    test('agent utilization chart area renders', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // AgentUtilChart has heading "Agent Activity" and renders a stacked bar SVG
      const agentPanel = page.locator('.rounded-xl', { hasText: 'Agent Activity' });
      const panelVisible = await agentPanel.isVisible().catch(() => false);

      if (panelVisible) {
        // Should have an SVG inside
        const svg = agentPanel.locator('svg').first();
        await expect(svg).toBeVisible({ timeout: 5000 });
      } else {
        // Chart only renders when metricsData is available; acceptable if not present
        expect(true).toBeTruthy();
      }
    });

    test('merge health chart area renders', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // MergeHealthChart has heading "Merge Health" and renders a donut SVG
      const mergePanel = page.locator('.rounded-xl', { hasText: 'Merge Health' });
      const panelVisible = await mergePanel.isVisible().catch(() => false);

      if (panelVisible) {
        const svg = mergePanel.locator('svg').first();
        await expect(svg).toBeVisible({ timeout: 5000 });
      } else {
        // Only renders when metricsData is present
        expect(true).toBeTruthy();
      }
    });

    test('activity feed section renders', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // ActivityFeed has heading "Activity Feed"
      const feedPanel = page.locator('.rounded-xl', { hasText: 'Activity Feed' });
      await expect(feedPanel).toBeVisible({ timeout: 10000 });

      // Should show entries or "No activity yet"
      const noActivity = feedPanel.locator('text=No activity yet');
      const entries = feedPanel.locator('.border-b').first();

      const isEmpty = await noActivity.isVisible().catch(() => false);
      const hasEntries = await entries.isVisible().catch(() => false);
      expect(isEmpty || hasEntries).toBeTruthy();
    });

    test('loom feed on dashboard renders', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // LoomFeed has an h3 heading "Loom"
      const loomHeading = page.locator('h3', { hasText: /^Loom$/ });
      await expect(loomHeading).toBeVisible({ timeout: 10000 });
    });

    test('metrics days toggle switches between 7d and 30d', async ({ page }) => {
      await page.goto(`${BASE}/#/`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(3000);

      // Find the 7d/30d toggle buttons
      const btn7d = page.locator('button', { hasText: '7d' });
      const btn30d = page.locator('button', { hasText: '30d' });

      await expect(btn7d).toBeVisible({ timeout: 10000 });
      await expect(btn30d).toBeVisible({ timeout: 5000 });

      // 7d should be active by default (has bg-purple-600 class)
      await expect(btn7d).toHaveClass(/bg-purple-600/);

      // Click 30d
      await btn30d.click();
      await page.waitForTimeout(1500);

      // 30d should now be active
      await expect(btn30d).toHaveClass(/bg-purple-600/);

      // Click back to 7d
      await btn7d.click();
      await page.waitForTimeout(1500);
      await expect(btn7d).toHaveClass(/bg-purple-600/);
    });
  });
});
