import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Project edit via API', () => {
  test('edit project description and restore original', async ({ request }) => {
    // Get current project
    const getRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}`);
    expect(getRes.status()).toBe(200);
    const original = await getRes.json();
    const originalDescription = original.description || '';

    // Update description
    const updateRes = await request.put(`${BASE}/api/projects/${PROJECT_ID}`, {
      data: { description: 'E2E test edit' },
    });
    expect(updateRes.status()).toBeLessThan(300);
    const updated = await updateRes.json();
    expect(updated.description).toBe('E2E test edit');

    // Restore original
    await request.put(`${BASE}/api/projects/${PROJECT_ID}`, {
      data: { description: originalDescription },
    });
  });

  test('project update response has expected structure', async ({ request }) => {
    const getRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}`);
    expect(getRes.status()).toBe(200);
    const body = await getRes.json();
    expect(body).toHaveProperty('id');
    expect(body).toHaveProperty('name');
    expect(body).toHaveProperty('directory');
    expect(body).toHaveProperty('description');
  });
});
