import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

let createdOverrideId: string | null = null;

async function goToProjectTab(page: any, tabName: string) {
  await page.goto('/#/projects');
  const tile = page.locator('.cursor-pointer h3').first();
  await expect(tile).toBeVisible({ timeout: 10000 });
  await tile.click();
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe.serial('Routing tab interactions', () => {
  test('Routing tab renders with suggestions section', async ({ page }) => {
    await goToProjectTab(page, 'Routing');

    const heading = page.locator('h2', { hasText: 'Routing Suggestions' });
    await expect(heading).toBeVisible({ timeout: 10000 });
  });

  test('Detect Patterns button exists', async ({ page }) => {
    await goToProjectTab(page, 'Routing');

    const detectButton = page.locator('button', { hasText: /Detect Patterns/ });
    await expect(detectButton).toBeVisible({ timeout: 10000 });
  });

  test('Detect Patterns triggers without error', async ({ page }) => {
    await goToProjectTab(page, 'Routing');

    const detectButton = page.locator('button', { hasText: /Detect Patterns/ });
    await expect(detectButton).toBeVisible({ timeout: 10000 });
    await detectButton.click();

    // Button text changes to "Scanning..." while running
    await expect(detectButton).toContainText(/Scanning|Detect Patterns/, { timeout: 15000 });

    // Wait for it to finish (button returns to "Detect Patterns")
    await expect(detectButton).toHaveText('Detect Patterns', { timeout: 30000 });

    // No error message should appear
    const errorDiv = page.locator('.text-red-400');
    await expect(errorDiv).toHaveCount(0);
  });

  test('Create routing override via API and verify in UI', async ({ page, request }) => {
    // Create override via API
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/routing-overrides`, {
      data: {
        role: 'Senior Coder',
        task_type: 'implementation',
        to_model: 'claude-sonnet-4-6',
        to_tier: 4,
        reason: 'E2E test override',
        confidence: 0.85,
        evidence: 'test',
        observations: 10,
      },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    createdOverrideId = body.id ?? null;

    // Navigate to Routing tab and verify the override appears
    await goToProjectTab(page, 'Routing');

    await expect(async () => {
      const role = page.locator('text=Senior Coder');
      await expect(role.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });

    // Verify additional details
    await expect(page.locator('text=implementation').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=E2E test override').first()).toBeVisible({ timeout: 5000 });
    await expect(page.locator('text=claude-sonnet-4-6').first()).toBeVisible({ timeout: 5000 });
  });

  test('Clean up test override via API', async ({ request }) => {
    // Reject the override first (moves it out of suggested state)
    if (createdOverrideId) {
      const rejectRes = await request.post(`${BASE}/api/routing-overrides/${createdOverrideId}/reject`);
      expect(rejectRes.ok()).toBeTruthy();
    }

    // Verify it was rejected by listing overrides
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/routing-overrides`);
    expect(listRes.ok()).toBeTruthy();
    const overrides = await listRes.json();
    const testOverride = overrides.find((o: any) => o.id === createdOverrideId);
    if (testOverride) {
      expect(testOverride.status).toBe('rejected');
    }
  });
});
