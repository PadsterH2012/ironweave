import { test, expect } from '@playwright/test';

test.describe('Agents', () => {
  test('page loads at /#/agents', async ({ page }) => {
    await page.goto('/#/agents');

    const heading = page.locator('text=/agents/i').first();
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('shows agent list or empty state', async ({ page }) => {
    await page.goto('/#/agents');

    // Either agent entries or a "No agents" empty state
    const content = page.locator('text=/agent|no agents|empty/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });
});
