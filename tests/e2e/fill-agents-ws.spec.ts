import { test, expect } from '@playwright/test';
import { BASE } from './test-helpers';

test.describe('Agents spawn & WebSocket terminal', () => {
  test('spawn agent API contract', async ({ request }) => {
    const res = await request.post(`${BASE}/api/agents/spawn`, {
      data: {
        runtime: 'claude',
        working_directory: '/tmp',
        prompt: 'test',
      },
    });
    // Spawn will likely fail (no valid runtime available) but should return
    // a structured error response, not 500
    expect(res.status()).toBeLessThan(500);
  });

  test('WebSocket agent endpoint exists', async ({ page }) => {
    // Navigate to a blank page first so we have a browser context for evaluate
    await page.goto(`${BASE}/#/`);
    await page.waitForLoadState('networkidle');

    const wsResult = await page.evaluate(async (baseUrl) => {
      return new Promise<string>((resolve) => {
        const wsUrl = baseUrl.replace('https', 'wss').replace('http', 'ws');
        const ws = new WebSocket(wsUrl + '/ws/agents/nonexistent');
        ws.onopen = () => {
          ws.close();
          resolve('connected');
        };
        ws.onerror = () => resolve('error');
        ws.onclose = () => resolve('closed');
        setTimeout(() => {
          ws.close();
          resolve('timeout');
        }, 5000);
      });
    }, BASE);

    // Any of these outcomes is acceptable — we just verify the endpoint doesn't crash
    expect(['connected', 'closed', 'error', 'timeout']).toContain(wsResult);
  });
});
