import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Files — diff viewer & restore snapshot', () => {
  test('sync diff API contract', async ({ request }) => {
    const historyRes = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/sync/history`
    );

    if (historyRes.status() === 404) {
      test.skip(true, 'Sync history endpoint not available');
      return;
    }
    expect(historyRes.status()).toBeLessThan(500);

    if (!historyRes.ok()) return;

    const snapshots = await historyRes.json();
    if (!Array.isArray(snapshots) || snapshots.length === 0) {
      test.skip(true, 'No sync snapshots available');
      return;
    }

    // Get diff for the first available snapshot
    const changeId = snapshots[0].id || snapshots[0].change_id;
    expect(changeId).toBeTruthy();

    const diffRes = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/sync/diff/${changeId}`
    );
    expect(diffRes.status()).toBeLessThan(500);
  });

  test('restore API returns 400 or 404 for nonexistent change', async ({ request }) => {
    const res = await request.post(
      `${BASE}/api/projects/${PROJECT_ID}/sync/restore`,
      {
        data: { change_id: 'nonexistent' },
      }
    );

    if (res.status() === 404) {
      // Endpoint not implemented — that's acceptable
      return;
    }

    // Should return 400 or 404 for invalid change_id, not 500
    expect(res.status()).toBeLessThan(500);
    expect([400, 404, 422]).toContain(res.status());
  });
});
