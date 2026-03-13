import { test, expect } from '@playwright/test';
import { login } from './helpers';

test.describe('Agents', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('spawns an agent and verifies terminal panel appears', async ({ page }) => {
    await page.goto('/agents');
    await page.click('[data-testid="spawn-agent"]');

    await page.fill('[data-testid="agent-runtime"]', 'node');
    await page.fill('[data-testid="agent-prompt"]', 'Fix the login page styling');
    await page.click('[data-testid="submit-agent"]');

    await expect(page.locator('[data-testid="terminal-panel"]')).toBeVisible();
  });

  test('stops an agent and verifies terminal panel is removed', async ({ page }) => {
    await page.goto('/agents');

    // Assume an agent is already running
    await page.click('[data-testid="stop-agent"]');

    await expect(page.locator('[data-testid="terminal-panel"]')).not.toBeVisible();
  });
});
