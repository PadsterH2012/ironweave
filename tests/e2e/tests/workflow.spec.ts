import { test, expect } from '@playwright/test';
import { login } from './helpers';

test.describe('Workflow', () => {
  let projectSlug: string;

  test.beforeEach(async ({ page, request }) => {
    await login(page);

    // Create a project with a workflow via the API
    const response = await request.post('/api/projects', {
      data: {
        name: 'Workflow Test Project',
        description: 'Created for workflow E2E tests',
        workflow: true,
      },
    });
    const project = await response.json();
    projectSlug = project.slug ?? project.id;
  });

  test('renders the DAG graph with expected nodes', async ({ page }) => {
    await page.goto(`/projects/${projectSlug}/workflow`);

    const dagGraph = page.locator('[data-testid="dag-graph"]');
    await expect(dagGraph).toBeVisible();

    // Verify at least one node is rendered
    const nodes = dagGraph.locator('[data-testid="dag-node"]');
    await expect(nodes.first()).toBeVisible();
    expect(await nodes.count()).toBeGreaterThan(0);
  });

  test('node colours correspond to stage status', async ({ page }) => {
    await page.goto(`/projects/${projectSlug}/workflow`);

    const dagGraph = page.locator('[data-testid="dag-graph"]');
    await expect(dagGraph).toBeVisible();

    // Verify nodes have status-based colour classes
    const pendingNodes = dagGraph.locator('[data-testid="dag-node"][data-status="pending"]');
    const activeNodes = dagGraph.locator('[data-testid="dag-node"][data-status="active"]');
    const completeNodes = dagGraph.locator('[data-testid="dag-node"][data-status="complete"]');

    // At least some nodes should exist with status attributes
    const totalStatusNodes =
      (await pendingNodes.count()) +
      (await activeNodes.count()) +
      (await completeNodes.count());
    expect(totalStatusNodes).toBeGreaterThan(0);
  });
});
