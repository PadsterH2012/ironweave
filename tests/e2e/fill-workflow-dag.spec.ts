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
    // Create a workflow definition with a 2-stage DAG
    const ts = Date.now();
    const wfRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/workflows`, {
      data: {
        project_id: PROJECT_ID,
        name: `E2E DAG Test ${ts}`,
        stages: [
          {
            id: 'stage-1',
            name: 'Build',
            runtime: 'claude',
            prompt: 'Build step',
            depends_on: [],
            is_manual_gate: false,
          },
          {
            id: 'stage-2',
            name: 'Deploy',
            runtime: 'claude',
            prompt: 'Deploy step',
            depends_on: ['stage-1'],
            is_manual_gate: false,
          },
        ],
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
      `${BASE}/api/projects/${PROJECT_ID}/workflows/${workflowId}/instances`,
      { data: { workflow_id: workflowId } }
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
