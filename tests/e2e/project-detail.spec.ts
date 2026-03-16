import { test, expect } from '@playwright/test';

test.describe('Project Detail', () => {
  let projectId: string | null = null;

  test.beforeAll(async ({ request }) => {
    // Fetch the project list to find a valid project ID
    try {
      const res = await request.get('/api/projects');
      if (res.ok()) {
        const data = await res.json();
        if (Array.isArray(data) && data.length > 0) {
          projectId = data[0].id;
        }
      }
    } catch {
      // No projects available — tests will be skipped
    }
  });

  test('all tabs are present', async ({ page }) => {
    test.skip(!projectId, 'No projects exist — skipping project detail tests');

    await page.goto(`/#/projects/${projectId}`, { waitUntil: 'networkidle', timeout: 10000 });

    const expectedTabs = [
      'Teams',
      'Issues',
      'Workflows',
      'Merge Queue',
      'Loom',
      'Files',
      'Prompts',
      'Quality',
      'Costs',
      'Coordinator',
      'Routing',
      'Tests',
      'Settings',
    ];

    for (const tabLabel of expectedTabs) {
      const tab = page.locator('button', { hasText: tabLabel });
      await expect(tab).toBeVisible({ timeout: 5000 });
    }
  });

  test('dispatch status badges render', async ({ page }) => {
    test.skip(!projectId, 'No projects exist — skipping project detail tests');

    await page.goto(`/#/projects/${projectId}`, { waitUntil: 'networkidle', timeout: 10000 });

    // The dispatch status area should show some status indicator
    const statusArea = page.locator('text=/dispatch|paused|active|running/i');
    await expect(statusArea.first()).toBeVisible({ timeout: 10000 });
  });
});
