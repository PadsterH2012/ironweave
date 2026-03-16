import { test, expect } from '@playwright/test';

const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';
const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const workflowName = `E2E Workflow ${Date.now()}`;

let createdDefId: string | undefined;
let createdInstanceId: string | undefined;
let teamId: string | undefined;

async function goToProjectTab(page: any, tabName: string) {
  await page.goto('/#/projects');
  const tile = page.locator('.cursor-pointer h3').first();
  await expect(tile).toBeVisible({ timeout: 10000 });
  await tile.click();
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: new RegExp(`^${tabName}$`) });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe.serial('Workflow definition and instance lifecycle', () => {
  test('Workflows tab renders with definitions', async ({ page }) => {
    await goToProjectTab(page, 'Workflows');

    // Verify the Workflows heading loads
    const heading = page.locator('h3', { hasText: 'Workflows' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Verify either definition list or empty state is visible
    const content = page.locator('text=No workflow definitions found').or(
      page.locator('.text-sm.font-medium.text-gray-200').first()
    );
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('Create workflow definition via API then verify in UI', async ({ page, request }) => {
    // Fetch the team ID first
    const teamsRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(teamsRes.ok()).toBeTruthy();
    const teams = await teamsRes.json();
    expect(teams.length).toBeGreaterThan(0);
    teamId = teams[0].id;

    // Create workflow definition via API
    const dagJson = JSON.stringify({
      stages: [
        { id: 'stage1', name: 'Build', role: 'senior_coder', deps: [] },
        { id: 'stage2', name: 'Test', role: 'senior_tester', deps: ['stage1'] },
      ],
    });

    const createRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/workflows`, {
      data: {
        name: workflowName,
        project_id: PROJECT_ID,
        team_id: teamId,
        dag: dagJson,
        version: 1,
      },
    });
    expect(createRes.ok()).toBeTruthy();
    const created = await createRes.json();
    createdDefId = created.id;

    // Navigate directly to the Ironweave project's Workflows tab
    await page.goto(`/#/projects/${PROJECT_ID}`);
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
    const wfTab = page.locator('button', { hasText: /^Workflows$/ });
    await expect(wfTab).toBeVisible({ timeout: 10000 });
    await wfTab.click();
    await expect(async () => {
      const defItem = page.locator(`text=${workflowName}`);
      await expect(defItem.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
  });

  test('Create workflow instance via API and verify via API', async ({ request }) => {
    expect(createdDefId).toBeDefined();

    // Create workflow instance via API
    const createRes = await request.post(
      `${BASE}/api/workflows/${createdDefId}/instances`,
      {
        data: {
          definition_id: createdDefId,
        },
      },
    );
    expect(createRes.ok()).toBeTruthy();
    const instance = await createRes.json();
    createdInstanceId = instance.id;

    // Verify instance exists via API
    const listRes = await request.get(`${BASE}/api/workflows/${createdDefId}/instances`);
    expect(listRes.ok()).toBeTruthy();
    const instances = await listRes.json();
    const found = instances.find((i: any) => i.id === createdInstanceId);
    expect(found).toBeDefined();
    expect(found.state).toBeDefined();
  });

  test('Clean up created data via API', async ({ request }) => {
    // Delete instance if created
    if (createdDefId && createdInstanceId) {
      const delInstanceRes = await request.delete(
        `${BASE}/api/workflows/${createdDefId}/instances/${createdInstanceId}`,
      );
      // Instance delete may not be supported; log but don't fail
      if (!delInstanceRes.ok()) {
        console.warn(`Instance delete returned ${delInstanceRes.status()}`);
      }
    }

    // Delete workflow definition
    if (createdDefId) {
      const delDefRes = await request.delete(
        `${BASE}/api/projects/${PROJECT_ID}/workflows/${createdDefId}`,
      );
      if (!delDefRes.ok()) {
        // Try alternative endpoint
        await request.delete(`${BASE}/api/workflows/${createdDefId}`);
      }
    }

    // Verify definition is gone
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/workflows`);
    if (listRes.ok()) {
      const remaining = await listRes.json();
      expect(remaining.find((d: any) => d.name === workflowName)).toBeUndefined();
    }
  });
});
