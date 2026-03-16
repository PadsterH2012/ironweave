import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Details API contracts', () => {
  test('GET /api/projects/{pid}/documents/intent returns document (get_or_create)', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/documents/intent`);
    expect(res.status()).toBe(200);
    const doc = await res.json();
    expect(doc).toHaveProperty('content');
    expect(doc).toHaveProperty('project_id');
  });

  test('PUT /api/projects/{pid}/documents/intent returns document + removals array', async ({ request }) => {
    const testContent = `API contract test ${Date.now()}`;
    const res = await request.put(`${BASE}/api/projects/${PROJECT_ID}/documents/intent`, {
      data: { content: testContent },
    });
    expect(res.status()).toBeLessThan(300);
    const result = await res.json();
    expect(result).toHaveProperty('document');
    expect(result.document.content).toBe(testContent);
    expect(result).toHaveProperty('removals');
    expect(Array.isArray(result.removals)).toBe(true);
  });

  test('GET /api/projects/{pid}/documents/intent/history returns document with version', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/documents/intent/history`);
    expect(res.status()).toBe(200);
    const doc = await res.json();
    expect(doc).toHaveProperty('version');
  });

  test('POST /api/projects/{pid}/documents/scan returns status object', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/documents/scan`);
    expect(res.status()).toBeLessThan(300);
    const result = await res.json();
    expect(result).toHaveProperty('status');
  });

  test('GET /api/projects/{pid}/documents/gaps returns {missing, undocumented} arrays', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/documents/gaps`);
    expect(res.status()).toBe(200);
    const gaps = await res.json();
    expect(gaps).toHaveProperty('missing');
    expect(gaps).toHaveProperty('undocumented');
    expect(Array.isArray(gaps.missing)).toBe(true);
    expect(Array.isArray(gaps.undocumented)).toBe(true);
  });
});
