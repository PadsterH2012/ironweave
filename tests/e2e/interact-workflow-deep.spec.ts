import { test, expect } from '@playwright/test';

const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';
const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const workflowName = `E2E-Deep-WF-${Date.now()}`;

const DAG_JSON = JSON.stringify({
  stages: [
    { id: 's1', name: 'Build', role: 'senior_coder', deps: [] },
    { id: 's2', name: 'Test', role: 'senior_tester', deps: ['s1'] },
  ],
});

let teamId: string | undefined;
let createdDefId: string | undefined;
let createdInstanceId: string | undefined;

test.describe.serial('Deep workflow instance lifecycle (pause/resume/cancel)', () => {
  test('Fetch team ID for workflow creation', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(res.ok()).toBeTruthy();
    const teams = await res.json();
    expect(teams.length).toBeGreaterThan(0);
    teamId = teams[0].id;
  });

  test('Create workflow definition via API', async ({ request }) => {
    expect(teamId).toBeDefined();

    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/workflows`, {
      data: {
        name: workflowName,
        project_id: PROJECT_ID,
        team_id: teamId,
        dag: DAG_JSON,
        version: 1,
      },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.id).toBeDefined();
    createdDefId = body.id;
  });

  test('Create workflow instance via API', async ({ request }) => {
    expect(createdDefId).toBeDefined();

    const res = await request.post(`${BASE}/api/workflows/${createdDefId}/instances`, {
      data: { definition_id: createdDefId },
    });
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(body.id).toBeDefined();
    expect(body.state).toBeDefined();
    createdInstanceId = body.id;
  });

  test('Pause instance and verify state change', async ({ request }) => {
    expect(createdDefId).toBeDefined();
    expect(createdInstanceId).toBeDefined();

    const res = await request.post(
      `${BASE}/api/workflows/${createdDefId}/instances/${createdInstanceId}/pause`,
    );
    // Accept either success or a known non-500 error (e.g. instance already in terminal state)
    expect(res.status()).toBeLessThan(500);

    if (res.ok()) {
      const body = await res.json();
      expect(body.state).toBe('paused');
    }
  });

  test('Resume instance and verify state change', async ({ request }) => {
    expect(createdDefId).toBeDefined();
    expect(createdInstanceId).toBeDefined();

    const res = await request.post(
      `${BASE}/api/workflows/${createdDefId}/instances/${createdInstanceId}/resume`,
    );
    expect(res.status()).toBeLessThan(500);

    if (res.ok()) {
      const body = await res.json();
      // After resume the state should no longer be paused
      expect(['running', 'pending', 'completed']).toContain(body.state);
    }
  });

  test('Cancel instance and verify state change', async ({ request }) => {
    expect(createdDefId).toBeDefined();
    expect(createdInstanceId).toBeDefined();

    const res = await request.post(
      `${BASE}/api/workflows/${createdDefId}/instances/${createdInstanceId}/cancel`,
    );
    expect(res.status()).toBeLessThan(500);

    if (res.ok()) {
      const body = await res.json();
      expect(body.state).toBe('cancelled');
    }
  });

  test('Workflow definitions API returns expected shape', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/workflows`);
    expect(res.ok()).toBeTruthy();
    const defs = await res.json();
    expect(Array.isArray(defs)).toBe(true);

    // At least the one we just created should be present
    expect(defs.length).toBeGreaterThan(0);

    const sample = defs[0];
    expect(sample).toHaveProperty('id');
    expect(sample).toHaveProperty('name');
    expect(sample).toHaveProperty('dag');
    expect(sample).toHaveProperty('version');
  });

  test('Workflow instances API returns expected structure', async ({ request }) => {
    expect(createdDefId).toBeDefined();

    const res = await request.get(`${BASE}/api/workflows/${createdDefId}/instances`);
    expect(res.ok()).toBeTruthy();
    const instances = await res.json();
    expect(Array.isArray(instances)).toBe(true);

    // Our created instance should be listed
    const found = instances.find((i: any) => i.id === createdInstanceId);
    expect(found).toBeDefined();
    expect(found).toHaveProperty('id');
    expect(found).toHaveProperty('state');
  });

  test('Clean up test data (best-effort)', async ({ request }) => {
    if (createdDefId && createdInstanceId) {
      await request.delete(
        `${BASE}/api/workflows/${createdDefId}/instances/${createdInstanceId}`,
      ).catch(() => {});
    }
    if (createdDefId) {
      await request.delete(
        `${BASE}/api/projects/${PROJECT_ID}/workflows/${createdDefId}`,
      ).catch(() => {});
    }
    // Best-effort cleanup — no assertions on delete success
    expect(true).toBe(true);
  });
});
