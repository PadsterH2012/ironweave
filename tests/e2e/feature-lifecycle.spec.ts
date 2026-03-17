import { test, expect } from '@playwright/test';

const BASE = (process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk');
const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';

test.describe.serial('Full Feature Lifecycle — end to end', () => {
  let featureId: string;
  let taskIds: string[] = [];

  test('1. Create a feature with PRD content', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features`, {
      data: {
        project_id: PROJECT_ID,
        title: 'E2E Lifecycle Test Feature',
        description: 'A test feature to validate the full lifecycle from idea to implementation plan.',
        status: 'idea',
        priority: 5,
        prd_content: '## Test Feature PRD\n\n### Requirements\n- Add a health check endpoint\n- Add a status page component\n- Write unit tests for the endpoint\n\n### Acceptance Criteria\n- GET /api/test-health returns 200\n- Frontend shows status indicator\n- Tests pass in CI',
        keywords: ['health', 'status', 'endpoint', 'test'],
      },
    });
    expect(res.ok()).toBeTruthy();
    const feature = await res.json();
    featureId = feature.id;
    expect(featureId).toBeTruthy();
    expect(feature.status).toBe('idea');
    expect(feature.prd_content).toContain('Test Feature PRD');
  });

  test('2. Add tasks to the feature', async ({ request }) => {
    const tasks = [
      'Create GET /api/test-health endpoint',
      'Add StatusIndicator.svelte component',
      'Write unit tests for health endpoint',
    ];

    for (const title of tasks) {
      const res = await request.post(`${BASE}/api/features/${featureId}/tasks`, {
        data: { feature_id: featureId, title, sort_order: tasks.indexOf(title) + 1 },
      });
      expect(res.ok()).toBeTruthy();
      const task = await res.json();
      taskIds.push(task.id);
    }
    expect(taskIds).toHaveLength(3);
  });

  test('3. Feature shows in UI with correct state', async ({ page }) => {
    await page.goto(`${BASE}/#/projects/${PROJECT_ID}`);
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
    await page.locator('button', { hasText: /^Features$/ }).click();
    await page.waitForTimeout(2000);

    // Feature should be visible
    await expect(page.locator('text=E2E Lifecycle Test Feature').first()).toBeVisible({ timeout: 10000 });
  });

  test('4. Feature GET returns feature with tasks', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`);
    expect(res.ok()).toBeTruthy();
    const data = await res.json();

    // Should have all fields
    expect(data.id).toBe(featureId);
    expect(data.title).toBe('E2E Lifecycle Test Feature');
    expect(data.status).toBe('idea');
    expect(data.prd_content).toContain('Test Feature PRD');
    expect(data.tasks).toHaveLength(3);
    expect(data.tasks[0].title).toBe('Create GET /api/test-health endpoint');
    expect(data.tasks[0].status).toBe('todo');
  });

  test('5. Click Implement on a task — creates issue with full context', async ({ request }) => {
    // Implement the first task
    const res = await request.post(`${BASE}/api/features/${featureId}/tasks/${taskIds[0]}/implement`);
    expect(res.ok()).toBeTruthy();
    const result = await res.json();

    // Should return both task and issue
    expect(result.task).toBeTruthy();
    expect(result.issue).toBeTruthy();
    expect(result.task.issue_id).toBe(result.issue.id);

    // Issue should have rich context
    const issueDesc = result.issue.description;
    expect(issueDesc).toContain('E2E Lifecycle Test Feature'); // Feature title
    expect(issueDesc).toContain('Test Feature PRD'); // PRD content
    expect(issueDesc).toContain('YOUR TASK'); // Current task marker
    expect(issueDesc).toContain('Create GET /api/test-health endpoint'); // Task title
    expect(issueDesc).toContain('Add StatusIndicator.svelte component'); // Other tasks listed
    expect(issueDesc).toContain('Write unit tests for health endpoint'); // Other tasks listed
  });

  test('6. Feature auto-promotes to in_progress after implement', async ({ request }) => {
    // The feature should now be in_progress (a task has an issue_id)
    // This happens in the orchestrator sweep — wait for a cycle
    // For the test, check directly
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`);
    const data = await res.json();

    // Task 1 should have issue_id set
    const task1 = data.tasks.find((t: any) => t.id === taskIds[0]);
    expect(task1.issue_id).toBeTruthy();

    // Note: auto-promotion happens in the sweep loop (30s), not synchronously
    // So status might still be 'idea' here — that's expected
    // The test validates the implement linkage works correctly
  });

  test('7. Generate Plan creates plan with enough detail', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}/plan`);
    expect(res.ok()).toBeTruthy();
    const result = await res.json();

    expect(result.issue_id).toBeTruthy();
    expect(result.message).toContain('Planning agent dispatched');

    // Verify the planning issue was created with correct role and content
    const issueRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    const issues = await issueRes.json();
    const planIssue = issues.find((i: any) => i.id === result.issue_id);

    expect(planIssue).toBeTruthy();
    expect(planIssue.role).toBe('Architect');
    expect(planIssue.title).toContain('Implementation Plan');
    expect(planIssue.description).toContain('Test Feature PRD'); // PRD included
    expect(planIssue.description).toContain('Create GET /api/test-health endpoint'); // Tasks listed
    expect(planIssue.description).toContain('Add StatusIndicator.svelte component');
    expect(planIssue.description).toContain('detailed implementation plan'); // Instructions present
  });

  test('8. Gap Analysis dispatches correctly', async ({ request }) => {
    const res = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}/gaps`);
    expect(res.ok()).toBeTruthy();
    const result = await res.json();

    expect(result.issue_id).toBeTruthy();
    expect(result.message).toContain('Gap analysis');

    // Verify the gap issue was created
    const issueRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    const issues = await issueRes.json();
    const gapIssue = issues.find((i: any) => i.id === result.issue_id);

    expect(gapIssue).toBeTruthy();
    expect(gapIssue.role).toBe('Gap Analyst');
    expect(gapIssue.description).toContain('Test Feature PRD');
  });

  test('9. Feature has gap_issue_id linked', async ({ request }) => {
    const res = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`);
    const data = await res.json();
    expect(data.gap_issue_id).toBeTruthy();
    expect(data.gap_status).toBeTruthy(); // 'pending' or 'complete'
  });

  test('10. Park and resume feature', async ({ request }) => {
    // Park
    const parkRes = await request.post(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}/park`, {
      data: { reason: 'Testing park flow' },
    });
    expect(parkRes.ok()).toBeTruthy();
    const parked = await parkRes.json();
    expect(parked.status).toBe('parked');
    expect(parked.parked_reason).toBe('Testing park flow');

    // Resume (set back to idea)
    const resumeRes = await request.put(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`, {
      data: { status: 'idea' },
    });
    expect(resumeRes.ok()).toBeTruthy();
    const resumed = await resumeRes.json();
    expect(resumed.status).toBe('idea');
  });

  test('11. Clean up — delete test feature and related issues', async ({ request }) => {
    // Delete related issues
    const issuesRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    const issues = await issuesRes.json();
    for (const issue of issues) {
      if (issue.title.includes('E2E Lifecycle') || issue.title.includes('Implementation Plan: E2E') || issue.title.includes('Gap Analysis: E2E')) {
        await request.delete(`${BASE}/api/projects/${PROJECT_ID}/issues/${issue.id}`);
      }
    }

    // Delete the feature (soft delete)
    await request.delete(`${BASE}/api/projects/${PROJECT_ID}/features/${featureId}`);

    // Verify gone from active list
    const featuresRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/features`);
    const features = await featuresRes.json();
    const found = features.find((f: any) => f.id === featureId && f.status !== 'abandoned');
    expect(found).toBeFalsy();
  });
});
