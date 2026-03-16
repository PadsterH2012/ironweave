import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe('Workflow DAG visualization', () => {
  let workflowId: string | null = null;
  let instanceId: string | null = null;

  test.afterEach(async ({ request }) => {
    // Clean up instance then workflow
    if (instanceId && workflowId) {
      await request.delete(
        `${BASE}/api/projects/${PROJECT_ID}/workflows/${workflowId}/instances/${instanceId}`
      );
      instanceId = null;
    }
    if (workflowId) {
      await request.delete(`${BASE}/api/projects/${PROJECT_ID}/workflows/${workflowId}`);
      workflowId = null;
    }
  });

  test('DAG container renders in workflow view', async ({ request, page }) => {
    // Get a team_id first
    const teamsRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    const teams = await teamsRes.json();
    if (!teams.length) { test.skip(true, 'No teams'); return; }
    const teamId = teams[0].id;

    // Create a workflow definition with a 2-stage DAG
    const ts = Date.now();
    const dagJson = JSON.stringify({
      stages: [
        { id: 's1', name: 'Build', role: 'senior_coder', deps: [] },
        { id: 's2', name: 'Test', role: 'senior_tester', deps: ['s1'] },
      ],
    });
    const wfRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/workflows`, {
      data: {
        name: `E2E DAG Test ${ts}`,
        project_id: PROJECT_ID,
        team_id: teamId,
        dag: dagJson,
        version: 1,
      },
    });

    if (wfRes.status() === 404) {
      test.skip(true, 'Workflow creation endpoint not available');
      return;
    }
    expect(wfRes.status()).toBeLessThan(300);
    const workflow = await wfRes.json();
    workflowId = workflow.id;
    expect(workflowId).toBeTruthy();

    // Optionally create an instance
    const instRes = await request.post(
      `${BASE}/api/workflows/${workflowId}/instances`,
      { data: { definition_id: workflowId } }
    );
    if (instRes.ok()) {
      const instance = await instRes.json();
      instanceId = instance.id;
    }

    // Navigate to the workflow view
    await page.goto(`${BASE}/#/projects/${PROJECT_ID}/workflows/${workflowId}`);
    await page.waitForLoadState('networkidle');

    // DagGraph.svelte renders a div with class containing bg-gray-950 and min-h-[400px]
    // Cytoscape injects a canvas element inside the container div
    const dagContainer = page.locator('div.bg-gray-950.min-h-\\[400px\\]').first();
    const cyCanvas = page.locator('canvas').first();
    const anyCyContainer = page.locator('[class*="cytoscape"]').first();

    // At least one of these selectors should match if the DAG rendered
    const containerVisible = await dagContainer
      .isVisible({ timeout: 8000 })
      .catch(() => false);
    const canvasVisible = await cyCanvas.isVisible({ timeout: 3000 }).catch(() => false);
    const cyContainerVisible = await anyCyContainer
      .isVisible({ timeout: 3000 })
      .catch(() => false);

    expect(containerVisible || canvasVisible || cyContainerVisible).toBe(true);
  });
});
