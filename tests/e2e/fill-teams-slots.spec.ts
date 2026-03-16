import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Team agent slot CRUD', () => {
  let teamId: string;

  test.beforeAll(async ({ request }) => {
    const teamsRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(teamsRes.status()).toBe(200);
    const teams = await teamsRes.json();
    expect(teams.length).toBeGreaterThan(0);
    teamId = teams[0].id;
  });

  test('list slots for a team returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/teams/${teamId}/slots`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('create, update, delete a slot', async ({ request }) => {
    // Create
    const createRes = await request.post(`${BASE}/api/teams/${teamId}/slots`, {
      data: { role: 'Senior Tester', runtime: 'claude', slot_order: 99 },
    });
    expect(createRes.status()).toBeLessThan(300);
    const slot = await createRes.json();
    expect(slot).toHaveProperty('id');
    const slotId = slot.id;

    // Update
    const updateRes = await request.put(`${BASE}/api/teams/${teamId}/slots/${slotId}`, {
      data: { role: 'Security Engineer' },
    });
    expect(updateRes.status()).toBeLessThan(300);
    const updated = await updateRes.json();
    expect(updated.role).toBe('Security Engineer');

    // Delete
    const deleteRes = await request.delete(`${BASE}/api/teams/${teamId}/slots/${slotId}`);
    expect(deleteRes.status()).toBeLessThan(300);

    // Verify gone
    const listRes = await request.get(`${BASE}/api/teams/${teamId}/slots`);
    const slots = await listRes.json();
    const found = slots.find((s: { id: string }) => s.id === slotId);
    expect(found).toBeUndefined();
  });
});
