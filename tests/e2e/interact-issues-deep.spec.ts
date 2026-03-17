import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();
const issueTitle = `E2E Deep Issue ${Date.now()}`;
let createdIssueId: string | null = null;

async function goToIssuesTab(page: any) {
  await page.goto(`/#/projects/${PROJECT_ID}`);
  await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
  const tab = page.locator('button', { hasText: /^Issues$/ });
  await expect(tab).toBeVisible({ timeout: 10000 });
  await tab.click();
}

test.describe.serial('Deep Issues interaction on Ironweave project', () => {
  test('Issue board columns render', async ({ page }) => {
    await goToIssuesTab(page);

    // Verify all 4 column headers exist
    for (const label of ['Open', 'In Progress', 'Review', 'Closed']) {
      const header = page.locator('h3', { hasText: new RegExp(`^${label}$`) });
      await expect(header).toBeVisible({ timeout: 10000 });
    }
  });

  test('Create issue with type and priority', async ({ page }) => {
    await goToIssuesTab(page);

    // Wait for board to load
    await expect(page.locator('h3', { hasText: /^Open$/ })).toBeVisible({ timeout: 10000 });

    // Click "+ New Issue" to toggle form
    const createButton = page.locator('button', { hasText: /\+ New Issue/ });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Fill title
    const titleInput = page.locator('input[placeholder="Issue title"]');
    await expect(titleInput).toBeVisible({ timeout: 5000 });
    await titleInput.fill(issueTitle);

    // Select type "bug" from the type dropdown
    const typeSelect = page.locator('select').first();
    await typeSelect.selectOption('bug');

    // Set priority via range input
    const priorityInput = page.locator('#issue-priority');
    await priorityInput.fill('4');

    // Click "Create"
    const submitButton = page.locator('button', { hasText: /^Create$/ });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Wait for issue to appear on the board
    await expect(async () => {
      const issue = page.locator(`text=${issueTitle}`);
      await expect(issue.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });

    // Capture the created issue ID via API for subsequent tests
    const res = await page.request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    const issues = await res.json();
    const found = issues.find((i: any) => i.title === issueTitle);
    expect(found).toBeTruthy();
    createdIssueId = found.id;
  });

  test('Issue detail modal', async ({ page }) => {
    await goToIssuesTab(page);

    // Wait for the created issue to appear
    await expect(async () => {
      const issue = page.locator(`text=${issueTitle}`);
      await expect(issue.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });

    // Click the issue card to open the detail modal
    const issueCard = page.locator(`text=${issueTitle}`).first();
    await issueCard.click();

    // Verify the modal opens with the issue title displayed in the h2 heading
    const modalTitle = page.locator('.fixed h2', { hasText: issueTitle });
    await expect(modalTitle).toBeVisible({ timeout: 5000 });
  });

  test('Change issue status via API', async ({ page, request }) => {
    expect(createdIssueId).toBeTruthy();

    // PATCH issue status to in_progress
    const patchRes = await request.patch(
      `${BASE}/api/projects/${PROJECT_ID}/issues/${createdIssueId}`,
      { data: { status: 'in_progress' } }
    );
    expect(patchRes.ok()).toBeTruthy();

    // Navigate to Issues tab and verify issue moved to "In Progress" column
    await goToIssuesTab(page);

    // The "In Progress" column should contain the issue
    await expect(async () => {
      // Find the In Progress column (second column in the grid)
      const inProgressColumn = page.locator('.grid > div').nth(1);
      const issue = inProgressColumn.locator(`text=${issueTitle}`);
      await expect(issue).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
  });

  test('Delete issue via API', async ({ page, request }) => {
    expect(createdIssueId).toBeTruthy();

    const delRes = await request.delete(
      `${BASE}/api/projects/${PROJECT_ID}/issues/${createdIssueId}`
    );
    expect(delRes.ok()).toBeTruthy();

    // Navigate to Issues tab and verify the issue is gone
    await goToIssuesTab(page);

    await expect(async () => {
      const issue = page.locator(`text=${issueTitle}`);
      await expect(issue).toHaveCount(0);
    }).toPass({ timeout: 15000, intervals: [2000] });

    createdIssueId = null;
  });

  test('Issues API contracts', async ({ request }) => {
    // GET issues list returns array
    const listRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    expect(listRes.ok()).toBeTruthy();
    const listData = await listRes.json();
    expect(Array.isArray(listData)).toBeTruthy();

    // GET ready issues returns array
    const readyRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues/ready`);
    expect(readyRes.ok()).toBeTruthy();
    const readyData = await readyRes.json();
    expect(Array.isArray(readyData)).toBeTruthy();
  });

  test('Issue children API', async ({ request }) => {
    // Create a parent issue
    const createRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/issues`, {
      data: {
        project_id: PROJECT_ID,
        title: `E2E Parent Issue ${Date.now()}`,
        description: 'Temporary parent for children API test',
        issue_type: 'task',
        priority: 3,
      },
    });
    expect(createRes.ok()).toBeTruthy();
    const parentIssue = await createRes.json();
    const parentId = parentIssue.id;

    try {
      // GET children endpoint returns array
      const childrenRes = await request.get(
        `${BASE}/api/projects/${PROJECT_ID}/issues/${parentId}/children`
      );
      expect(childrenRes.ok()).toBeTruthy();
      const childrenData = await childrenRes.json();
      expect(Array.isArray(childrenData)).toBeTruthy();
    } finally {
      // Clean up parent issue
      await request.delete(`${BASE}/api/projects/${PROJECT_ID}/issues/${parentId}`);
    }
  });
});
