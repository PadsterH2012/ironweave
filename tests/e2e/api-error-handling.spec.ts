import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('API error handling', () => {
  test('invalid project ID returns 404', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/nonexistent-id`);
    expect(response.status()).toBe(404);
  });

  test('invalid test run ID returns 404', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/tests/runs/nonexistent`);
    expect(response.status()).toBe(404);
  });

  test('invalid role returns 404', async ({ request }) => {
    const response = await request.get(`${BASE}/api/roles/nonexistent-role`);
    expect(response.status()).toBe(404);
  });

  test('invalid setting returns 404', async ({ request }) => {
    const response = await request.get(`${BASE}/api/settings/nonexistent-key`);
    expect(response.status()).toBe(404);
  });
});
