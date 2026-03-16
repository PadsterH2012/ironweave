import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('Cron expression validation', () => {
  test('valid 7-field cron is accepted', async ({ request }) => {
    const createRes = await request.post(`${BASE}/api/dispatch/schedules`, {
      data: {
        scope: 'global',
        cron_expression: '0 0 9 * * 1-5 *',
        action: 'pause',
        timezone: 'UTC',
      },
    });
    expect([200, 201]).toContain(createRes.status());
    const schedule = await createRes.json();

    // Clean up
    if (schedule.id) {
      await request.delete(`${BASE}/api/dispatch/schedules/${schedule.id}`);
    }
  });

  test('invalid cron is rejected with 4xx', async ({ request }) => {
    const createRes = await request.post(`${BASE}/api/dispatch/schedules`, {
      data: {
        scope: 'global',
        cron_expression: 'not-a-cron',
        action: 'pause',
        timezone: 'UTC',
      },
    });
    expect([400, 422]).toContain(createRes.status());
  });
});
