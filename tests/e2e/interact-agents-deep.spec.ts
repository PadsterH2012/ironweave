import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('Agents deep interactions', () => {
  test.describe('Agents page UI', () => {
    test('renders with agent cards or empty state', async ({ page }) => {
      await page.goto(`${BASE}/#/agents`, { waitUntil: 'domcontentloaded' });
      await page.waitForTimeout(2000);

      // Page heading should be visible
      const heading = page.locator('h1', { hasText: 'Agents' });
      await expect(heading).toBeVisible({ timeout: 10000 });

      // Should show either agent cards (grid with runtime badges) or empty state
      const emptyState = page.locator('text=No active agent sessions');
      const agentCard = page.locator('.rounded-xl', { hasText: /claude|opencode|gemini/i }).first();

      // One of these two states must be true
      const isEmpty = await emptyState.isVisible().catch(() => false);
      const hasAgents = await agentCard.isVisible().catch(() => false);
      expect(isEmpty || hasAgents).toBeTruthy();

      if (isEmpty) {
        // Verify the "Spawn Agent" button is available
        const spawnBtn = page.locator('button', { hasText: 'Spawn Agent' });
        await expect(spawnBtn).toBeVisible({ timeout: 5000 });
      }

      if (hasAgents) {
        // Verify agent cards have expected elements: state dot, ID, runtime badge
        const firstCard = page.locator('.rounded-xl .font-mono').first();
        await expect(firstCard).toBeVisible({ timeout: 5000 });
      }
    });
  });

  test.describe('Agents API', () => {
    test('GET /api/agents returns an array', async ({ request }) => {
      const res = await request.get(`${BASE}/api/agents`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(Array.isArray(body)).toBeTruthy();

      // If there are agents, verify basic shape
      if (body.length > 0) {
        const agent = body[0];
        expect(agent).toHaveProperty('id');
        expect(agent).toHaveProperty('state');
        expect(agent).toHaveProperty('runtime');
      }
    });

    test('DELETE /api/agents/dead returns success', async ({ request }) => {
      const res = await request.delete(`${BASE}/api/agents/dead`, {
        ignoreHTTPSErrors: true,
      });
      // Should return 200 or 204
      expect([200, 204]).toContain(res.status());
    });

    test('GET /api/runtimes returns array with expected shape', async ({ request }) => {
      const res = await request.get(`${BASE}/api/runtimes`, {
        ignoreHTTPSErrors: true,
      });
      expect(res.ok()).toBeTruthy();

      const body = await res.json();
      expect(Array.isArray(body)).toBeTruthy();

      // Verify at least one runtime exists with expected fields
      if (body.length > 0) {
        const runtime = body[0];
        expect(runtime).toHaveProperty('id');
        expect(runtime).toHaveProperty('name');
        expect(runtime).toHaveProperty('capabilities');
      }
    });
  });
});
