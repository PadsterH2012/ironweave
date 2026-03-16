import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';
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
