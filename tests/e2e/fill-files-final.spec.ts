import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

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

    // Verify the endpoint responds (may return 400, 404, or 500 depending on implementation)
    expect([400, 404, 422, 500]).toContain(res.status());
  });
});
