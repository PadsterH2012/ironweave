import { test, expect } from '@playwright/test';
import { BASE } from './test-helpers';

const issueTitle = `E2E Test Issue ${Date.now()}`;

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

test.describe.serial('Issue CRUD on Ironweave project', () => {
  test('create a new issue', async ({ page }) => {
    await goToProjectTab(page, 'Issues');

    // Wait for the board columns to render
    await expect(page.locator('text=Open').first()).toBeVisible({ timeout: 10000 });

    // Click "+ New Issue" button in the Open column
    const createButton = page.locator('button', { hasText: /\+ New Issue/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Fill title (placeholder is "Issue title")
    const titleInput = page.locator('input[placeholder="Issue title"]');
    await expect(titleInput).toBeVisible({ timeout: 5000 });
    await titleInput.fill(issueTitle);

    // Click "Create" submit button
    const submitButton = page.locator('button', { hasText: /^Create$/ });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Wait for issue to appear (board refreshes every 5s)
    await expect(async () => {
      const issue = page.locator(`text=${issueTitle}`);
      await expect(issue.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
  });

  test('issue appears in the board', async ({ page }) => {
    await goToProjectTab(page, 'Issues');

    // Verify the test issue is visible
    await expect(async () => {
      const issue = page.locator(`text=${issueTitle}`);
      await expect(issue.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
  });

  test('delete the test issue via API', async ({ request }) => {
    // Use API to clean up — more reliable than UI deletion
    // BASE imported from test-helpers
    const projectsRes = await request.get(`${BASE}/api/projects`);
    const projects = await projectsRes.json();
    const pid = projects[0]?.id;
    if (!pid) return;

    const issuesRes = await request.get(`${BASE}/api/projects/${pid}/issues`);
    const issues = await issuesRes.json();
    const testIssue = issues.find((i: any) => i.title === issueTitle);
    if (testIssue) {
      await request.delete(`${BASE}/api/projects/${pid}/issues/${testIssue.id}`);
    }
  });
});
