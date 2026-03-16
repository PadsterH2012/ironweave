import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('POST/PUT validation returns 4xx', () => {
  test('create project with missing fields returns 400/422', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects`, { data: {} });
    expect([400, 422]).toContain(res.status());
  });

  test('create issue with missing title returns 400/422', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/issues`, {
      data: { project_id: PROJECT_ID },
    });
    expect([400, 422]).toContain(res.status());
  });

  test('create team with missing name returns 400/422', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/teams`, {
      data: {},
    });
    expect([400, 422]).toContain(res.status());
  });

  test('update setting with invalid body returns 400/422', async ({ request }) => {
    const res = await request.put(`${BASE}/api/settings/test_key`, {
      data: {},
    });
    expect([400, 422]).toContain(res.status());
  });

  test('create schedule with invalid timezone returns error', async ({ request }) => {
    const res = await request.post(`${BASE}/api/dispatch/schedules`, {
      data: {
        scope: 'global',
        cron_expression: '0 0 9 * * 1-5 *',
        action: 'pause',
        timezone: 'Invalid/Zone',
      },
    });
    expect(res.status()).toBeGreaterThanOrEqual(400);
    expect(res.status()).toBeLessThan(500);
  });
});
