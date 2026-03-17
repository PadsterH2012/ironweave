import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

test.describe.serial('Deep Teams interaction on Ironweave project', () => {
  test('Activate/deactivate team via API', async ({ request }) => {
    // GET all teams for the project
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(listRes.ok()).toBeTruthy();
    const teams = await listRes.json();
    expect(Array.isArray(teams)).toBeTruthy();
    expect(teams.length).toBeGreaterThan(0);

    const team = teams[0];
    const teamId = team.id;

    // Deactivate the team
    const deactivateRes = await request.put(
      `${BASE}/api/projects/${PROJECT_ID}/teams/${teamId}/deactivate`
    );
    expect(deactivateRes.ok()).toBeTruthy();
    const deactivated = await deactivateRes.json();
    expect(deactivated.is_active).toBe(false);

    // Re-activate the team to restore state
    const activateRes = await request.put(
      `${BASE}/api/projects/${PROJECT_ID}/teams/${teamId}/activate`
    );
    expect(activateRes.ok()).toBeTruthy();
    const activated = await activateRes.json();
    expect(activated.is_active).toBe(true);
  });

  test('Team templates API', async ({ request }) => {
    const res = await request.get(`${BASE}/api/teams/templates`);
    expect(res.ok()).toBeTruthy();
    const templates = await res.json();
    expect(Array.isArray(templates)).toBeTruthy();
  });

  test('Team status API', async ({ request }) => {
    // GET all teams to find a valid team ID
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(listRes.ok()).toBeTruthy();
    const teams = await listRes.json();
    expect(teams.length).toBeGreaterThan(0);

    const teamId = teams[0].id;

    // GET team status
    const statusRes = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/teams/${teamId}/status`
    );
    expect(statusRes.ok()).toBeTruthy();
    const status = await statusRes.json();
    expect(status).toHaveProperty('is_active');
    expect(status).toHaveProperty('roles');
    expect(status).toHaveProperty('scaling');
  });
});
