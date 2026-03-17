import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

let createdPatternId: string;

test.describe('Knowledge API contract tests', () => {
  test('GET list returns array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/knowledge`);
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('POST create returns pattern', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/knowledge`, {
      data: {
        project_id: PROJECT_ID,
        pattern_type: 'solution',
        title: 'API Test',
        content: 'Test',
        source_type: 'manual',
        keywords: ['test'],
      },
    });
    expect([200, 201]).toContain(res.status());
    const body = await res.json();
    expect(body).toHaveProperty('id');
    expect(body).toHaveProperty('title');
    expect(body).toHaveProperty('pattern_type');
    createdPatternId = body.id;
  });

  test('POST search returns scored results', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/knowledge/search`, {
      data: { query: 'test' },
    });
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('GET cross-project returns array', async ({ request }) => {
    const res = await request.get(`${BASE}/api/knowledge/cross-project?query=test`);
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('PUT update works', async ({ request }) => {
    test.skip(!createdPatternId, 'No pattern created to update');
    const res = await request.put(
      `${BASE}/api/projects/${PROJECT_ID}/knowledge/${createdPatternId}`,
      { data: { title: 'Updated' } },
    );
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body.title).toBe('Updated');
  });

  test('DELETE removes pattern', async ({ request }) => {
    test.skip(!createdPatternId, 'No pattern created to delete');
    const res = await request.delete(
      `${BASE}/api/projects/${PROJECT_ID}/knowledge/${createdPatternId}`,
    );
    expect(res.status()).toBe(204);
  });

  test('POST extract returns count', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/knowledge/extract`);
    expect(res.status()).toBe(200);
    const body = await res.json();
    expect(body).toHaveProperty('extracted');
  });

  test('GET single pattern returns 404 for missing', async ({ request }) => {
    const res = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/knowledge/nonexistent`,
    );
    expect(res.status()).toBe(404);
  });
});
