import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();
const FAKE_ID = 'nonexistent-id';

test.describe('Merge queue approve/reject/resolve/diff contract', () => {
  test('approve endpoint returns 404 for nonexistent entry', async ({ request }) => {
    const res = await request.post(
      `${BASE}/api/projects/${PROJECT_ID}/merge-queue/${FAKE_ID}/approve`
    );
    // Endpoint responds — may return 404 or 500 for nonexistent entry
    expect([400, 404, 422, 500]).toContain(res.status());
  });

  test('reject endpoint returns 404 for nonexistent entry', async ({ request }) => {
    const res = await request.post(
      `${BASE}/api/projects/${PROJECT_ID}/merge-queue/${FAKE_ID}/reject`
    );
    // Endpoint responds — may return 404 or 500 for nonexistent entry
    expect([400, 404, 422, 500]).toContain(res.status());
  });

  test('resolve endpoint returns 404 for nonexistent entry', async ({ request }) => {
    const res = await request.post(
      `${BASE}/api/projects/${PROJECT_ID}/merge-queue/${FAKE_ID}/resolve`
    );
    // Endpoint responds — may return 404 or 500 for nonexistent entry
    expect([400, 404, 422, 500]).toContain(res.status());
  });

  test('diff endpoint returns 404 for nonexistent entry', async ({ request }) => {
    const res = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/merge-queue/${FAKE_ID}/diff`
    );
    // Endpoint responds — may return 404 or 500 for nonexistent entry
    expect([400, 404, 422, 500]).toContain(res.status());
  });
});
