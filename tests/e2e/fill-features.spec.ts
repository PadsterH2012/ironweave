import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

let createdFeatureId: string;
let createdTaskId: string;

test.describe('Features API contracts', () => {
  test('GET /api/projects/{pid}/features returns array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features`);
    expect(res.status()).toBe(200);
    const data = await res.json();
    expect(Array.isArray(data)).toBe(true);
  });

  test('POST create returns feature with id, title, status', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features`, {
      data: {
        project_id: PROJECT_ID,
        title: `Fill Feature ${Date.now()}`,
        description: 'API contract test',
      },
    });
    expect(res.status()).toBeLessThan(300);
    const feature = await res.json();
    expect(feature.id).toBeTruthy();
    expect(feature.title).toContain('Fill Feature');
    expect(feature.status).toBeTruthy();
    createdFeatureId = feature.id;
  });

  test('GET /api/projects/{pid}/features/{id} returns feature with tasks array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}`);
    expect(res.status()).toBe(200);
    const feature = await res.json();
    expect(feature.id).toBe(createdFeatureId);
    expect(Array.isArray(feature.tasks)).toBe(true);
  });

  test('PUT update works', async ({ request }) => {
    const res = await request.put(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}`, {
      data: { description: 'Updated description' },
    });
    expect(res.status()).toBeLessThan(300);
    const updated = await res.json();
    expect(updated.description).toBe('Updated description');
  });

  test('POST park sets status to parked', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}/park`, {
      data: { reason: 'testing park' },
    });
    expect(res.status()).toBeLessThan(300);
    const parked = await res.json();
    expect(parked.status).toBe('parked');
  });

  test('POST verify sets status to verified', async ({ request }) => {
    // First unpark by updating status back to implemented
    await request.put(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}`, {
      data: { status: 'implemented' },
    });

    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}/verify`);
    expect(res.status()).toBeLessThan(300);
    const verified = await res.json();
    expect(verified.status).toBe('verified');
  });

  test('DELETE soft-deletes (status→abandoned)', async ({ request }) => {
    const res = await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${createdFeatureId}`);
    expect(res.status()).toBeLessThan(300);
    const deleted = await res.json();
    expect(deleted.status).toBe('abandoned');
  });

  test('POST /api/projects/{pid}/features/import returns feature', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/import`, {
      data: { text: 'Feature idea\n- Task 1\n- Task 2' },
    });
    expect(res.status()).toBeLessThan(300);
    const feature = await res.json();
    expect(feature.id).toBeTruthy();

    // Clean up
    await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${feature.id}`);
  });

  test('GET /api/features/summary returns array of summary objects', async ({ request }) => {
    const res = await request.get(`${BASE}/api/features/summary`);
    expect(res.status()).toBe(200);
    const data = await res.json();
    expect(Array.isArray(data)).toBe(true);
  });

  test('POST /api/features/{fid}/tasks/{id}/implement creates linked issue', async ({ request }) => {
    // Create a fresh feature and task for this test
    const fRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features`, {
      data: {
        project_id: PROJECT_ID,
        title: `Implement Test ${Date.now()}`,
        description: 'For implement test',
      },
    });
    const feature = await fRes.json();

    const tRes = await request.post(`${BASE}/api/features/${feature.id}/tasks`, {
      data: { feature_id: feature.id, title: 'Task to implement' },
    });
    const task = await tRes.json();
    createdTaskId = task.id;

    const implRes = await request.post(`${BASE}/api/features/${feature.id}/tasks/${createdTaskId}/implement`);
    expect(implRes.status()).toBeLessThan(300);
    const result = await implRes.json();
    expect(result.task.issue_id).toBeTruthy();
    expect(result.issue.id).toBeTruthy();

    // Clean up
    await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${feature.id}`);
  });
});

test.describe('Feature gap analysis', () => {
  test('POST /api/projects/{pid}/features/{id}/gaps dispatches agent', async ({ request }) => {
    // Create a feature with tasks
    const fRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features`, {
      data: { project_id: PROJECT_ID, title: 'Gap Test Feature', description: 'Test gap analysis' },
    });
    expect(fRes.ok()).toBeTruthy();
    const feature = await fRes.json();

    await request.post(`${BASE}/api/features/${feature.id}/tasks`, {
      data: { feature_id: feature.id, title: 'database migrations' },
    });

    // Trigger gap analysis — now creates an issue
    const gapRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${feature.id}/gaps`);
    expect(gapRes.ok()).toBeTruthy();
    const result = await gapRes.json();
    expect(result.issue_id).toBeTruthy();
    expect(result.message).toContain('Gap analysis');

    // Clean up
    await request.delete(`${BASE}/api/projects/${PROJECT_ID}/issues/${result.issue_id}`);
    await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${feature.id}`);
  });
});
