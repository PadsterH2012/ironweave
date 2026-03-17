import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

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
