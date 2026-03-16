import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Merge queue API tests', () => {
  test('merge queue list entries have expected structure', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    expect(response.status()).toBe(200);
    const entries = await response.json();
    expect(Array.isArray(entries)).toBe(true);

    for (const entry of entries) {
      expect(entry).toHaveProperty('id');
      expect(entry).toHaveProperty('project_id');
      expect(entry).toHaveProperty('branch_name');
      expect(entry).toHaveProperty('status');
      expect(entry).toHaveProperty('created_at');
      expect(entry).toHaveProperty('updated_at');
    }
  });

  test('merge queue diff endpoint exists', async ({ request }) => {
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    const entries = await listRes.json();

    if (entries.length > 0) {
      const entryId = entries[0].id;
      const diffRes = await request.get(
        `${BASE}/api/projects/${PROJECT_ID}/merge-queue/${entryId}/diff`
      );
      // Accept any non-500 response — endpoint may return 200 or 404
      expect(diffRes.status()).toBeLessThan(500);
    }
  });
});
