import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

test.describe('App preview / app status', () => {
  test('App status API returns object with state field', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/app/status`);
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body).toHaveProperty('state');
    // state should be a known value (most likely "stopped" for a project without a running app)
    expect(typeof body.state).toBe('string');
    expect(body.state.length).toBeGreaterThan(0);
  });

  test('App status indicator shows on project detail page', async ({ page }) => {
    await page.goto(`/#/projects/${PROJECT_ID}`);
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });

    // The project detail page shows either "Open App" link or app status indicator
    // Look for the app status badge or the "Open App" link
    const appIndicator = page
      .locator('a', { hasText: 'Open App' })
      .or(page.locator('text=App Stopped'))
      .or(page.locator('text=App Running'))
      .or(page.locator('text=App Error'))
      .or(page.locator('text=No App'));

    // The indicator may take a moment to appear after the status API call completes
    await expect(appIndicator.first()).toBeVisible({ timeout: 15000 });
  });

  test('App start API returns a valid response (not 500)', async ({ request }) => {
    // We do NOT actually expect this to succeed — the project may not have
    // a runnable app. We only verify the API contract: it should respond
    // with a structured response, not an internal server error.
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/app/start`);

    // Accept 200 (started), 201 (created), or 400 (bad request / no runnable app)
    // but NOT 500 (internal server error)
    expect(res.status()).toBeLessThan(500);

    // If the response is JSON, verify it has expected shape
    const contentType = res.headers()['content-type'] || '';
    if (contentType.includes('application/json')) {
      const body = await res.json();
      // If we got a success response, it should have an AppStatus shape
      if (res.ok()) {
        expect(body).toHaveProperty('state');
      }
    }
  });
});
