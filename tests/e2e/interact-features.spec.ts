import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

let featureId: string;
let taskId: string;

async function goToFeaturesTab(page: any) {
  await page.goto(`${BASE}/#/projects/${PROJECT_ID}`, { waitUntil: 'networkidle', timeout: 15000 });
  const featuresTab = page.locator('button', { hasText: /^Features$/ });
  await expect(featuresTab).toBeVisible({ timeout: 10000 });
  await featuresTab.click();
  // Wait for the panel to render
  await expect(page.locator('.space-y-4').first()).toBeVisible({ timeout: 10000 });
}

test.describe.serial('Feature interactions', () => {
  test('create feature via API and verify in UI', async ({ page, request }) => {
    const timestamp = Date.now();
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features`, {
      data: {
        project_id: PROJECT_ID,
        title: `E2E Feature ${timestamp}`,
        description: 'Test feature',
      },
    });
    expect(res.status()).toBeLessThan(300);
    const feature = await res.json();
    featureId = feature.id;
    expect(featureId).toBeTruthy();

    await goToFeaturesTab(page);
    await expect(page.getByText(`E2E Feature ${timestamp}`)).toBeVisible({ timeout: 10000 });
  });

  test('add task to feature via API and verify in UI', async ({ page, request }) => {
    const res = await request.post(`${BASE}/api/features/${featureId}/tasks`, {
      data: {
        feature_id: featureId,
        title: 'E2E Task',
      },
    });
    expect(res.status()).toBeLessThan(300);
    const task = await res.json();
    taskId = task.id;
    expect(taskId).toBeTruthy();

    await goToFeaturesTab(page);
    // Expand the feature to see tasks
    const featureRow = page.getByText(`E2E Feature`, { exact: false }).first();
    await featureRow.click();
    await expect(page.getByText('E2E Task')).toBeVisible({ timeout: 10000 });
  });

  test('implement button creates issue', async ({ request }) => {
    const res = await request.post(`${BASE}/api/features/${featureId}/tasks/${taskId}/implement`);
    expect(res.status()).toBeLessThan(300);
    const result = await res.json();
    expect(result.issue_id).toBeTruthy();
  });

  test('park feature via API and verify status changes', async ({ page, request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}/park`, {
      data: { reason: 'testing' },
    });
    expect(res.status()).toBeLessThan(300);
    const parked = await res.json();
    expect(parked.status).toBe('parked');

    await goToFeaturesTab(page);
    // Click "Parked" filter to find our feature
    const parkedFilter = page.locator('button', { hasText: 'Parked' });
    await parkedFilter.click();
    await expect(page.getByText('parked').first()).toBeVisible({ timeout: 10000 });
  });

  test('clean up via API', async ({ request }) => {
    const res = await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`);
    expect(res.status()).toBeLessThan(300);
  });
});
