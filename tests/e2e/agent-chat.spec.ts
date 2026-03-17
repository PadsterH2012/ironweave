import { test, expect } from '@playwright/test';

const BASE = (process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk') + '/api';

// Helper: get or create a project + team for testing
async function getTestProjectAndTeam(request: any) {
  const projectsRes = await request.get(`${BASE}/projects`);
  const projects = await projectsRes.json();
  let project = projects[0];
  if (!project) {
    const createRes = await request.post(`${BASE}/projects`, {
      data: { name: 'agent-chat-test', directory: '/tmp/agent-chat-test', context: 'homelab' },
    });
    project = await createRes.json();
  }

  const teamsRes = await request.get(`${BASE}/projects/${project.id}/teams`);
  const teams = await teamsRes.json();
  let team = teams[0];
  if (!team) {
    const createRes = await request.post(`${BASE}/projects/${project.id}/teams`, {
      data: { name: 'test-team', project_id: project.id },
    });
    team = await createRes.json();
  }
  return { project, team };
}

test.describe('Agent Chat - Pending Questions API', () => {
  test('GET /api/projects/{pid}/loom/questions returns array', async ({ request }) => {
    const { project } = await getTestProjectAndTeam(request);
    const res = await request.get(`${BASE}/projects/${project.id}/loom/questions`);
    expect(res.ok()).toBeTruthy();
    const body = await res.json();
    expect(Array.isArray(body)).toBeTruthy();
  });

  test('POST answer creates answer entry linked to question', async ({ request }) => {
    const { project, team } = await getTestProjectAndTeam(request);

    // Create a question entry via loom
    const qRes = await request.post(`${BASE}/loom`, {
      data: {
        team_id: team.id,
        project_id: project.id,
        entry_type: 'question',
        content: 'What branch should I target?',
      },
    });
    expect(qRes.ok()).toBeTruthy();
    const question = await qRes.json();

    // Post an answer
    const aRes = await request.post(`${BASE}/loom/answer`, {
      data: {
        question_id: question.id,
        content: 'Target the main branch',
        team_id: team.id,
        project_id: project.id,
      },
    });
    expect(aRes.status()).toBe(201);
    const answer = await aRes.json();
    expect(answer.entry_type).toBe('answer');
    expect(answer.content).toContain(question.id);
    expect(answer.content).toContain('Target the main branch');
  });

  test('question appears in pending questions list', async ({ request }) => {
    const { project, team } = await getTestProjectAndTeam(request);

    // Create a question
    const qRes = await request.post(`${BASE}/loom`, {
      data: {
        team_id: team.id,
        project_id: project.id,
        entry_type: 'question',
        content: 'Should I refactor the auth module?',
      },
    });
    expect(qRes.ok()).toBeTruthy();
    const question = await qRes.json();

    // Fetch pending questions
    const listRes = await request.get(`${BASE}/projects/${project.id}/loom/questions`);
    expect(listRes.ok()).toBeTruthy();
    const questions = await listRes.json();
    const found = questions.find((q: any) => q.id === question.id);
    expect(found).toBeTruthy();
    expect(found.entry_type).toBe('question');
  });
});

test.describe('Agent Chat - UI', () => {
  async function goToProjectTab(page: any, tabName: string | RegExp) {
    await page.goto('/#/projects');
    const tile = page.locator('.cursor-pointer h3').first();
    await expect(tile).toBeVisible({ timeout: 10000 });
    await tile.click();
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });
    const tab = page.locator('button', { hasText: tabName });
    await expect(tab).toBeVisible({ timeout: 10000 });
    await tab.click();
  }

  test('question shows in Loom feed with reply button', async ({ page, request }) => {
    const { project, team } = await getTestProjectAndTeam(request);

    // Create a question entry
    await request.post(`${BASE}/loom`, {
      data: {
        team_id: team.id,
        project_id: project.id,
        entry_type: 'question',
        content: 'UI test question: which deploy strategy?',
      },
    });

    // Navigate to Loom tab
    await goToProjectTab(page, /^Loom/);

    // Verify question shows up
    const questionText = page.locator('text=UI test question: which deploy strategy?');
    await expect(questionText).toBeVisible({ timeout: 10000 });

    // Verify reply button exists
    const replyBtn = page.locator('text=Reply to this question').first();
    await expect(replyBtn).toBeVisible({ timeout: 5000 });
  });

  test('question entry shows orange color styling', async ({ page, request }) => {
    const { project, team } = await getTestProjectAndTeam(request);

    await request.post(`${BASE}/loom`, {
      data: {
        team_id: team.id,
        project_id: project.id,
        entry_type: 'question',
        content: 'Color test question entry',
      },
    });

    await goToProjectTab(page, /^Loom/);

    // Find the question icon with orange styling
    const orangeIcon = page.locator('.text-orange-400').first();
    await expect(orangeIcon).toBeVisible({ timeout: 10000 });
  });
});
