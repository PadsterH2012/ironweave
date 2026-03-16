import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('Prompt template role assignment and build preview', () => {
  test('create template, assign to role, build prompt, then clean up', async ({ request }) => {
    // 1. Create a prompt template
    const createRes = await request.post(`${BASE}/api/prompt-templates`, {
      data: {
        name: 'E2E Fill Test',
        template_type: 'role',
        content: 'You are a test agent.',
      },
    });
    expect(createRes.ok()).toBeTruthy();
    const template = await createRes.json();
    expect(template).toHaveProperty('id');
    const templateId = template.id;

    try {
      // 2. Assign template to a role
      const assignRes = await request.post(`${BASE}/api/prompt-templates/assignments`, {
        data: {
          role: 'Senior Coder',
          template_id: templateId,
          priority: 99,
        },
      });
      expect(assignRes.status()).toBeLessThan(300);
      const assignment = await assignRes.json();
      const assignmentId = assignment.id;

      try {
        // 3. Build prompt for role
        const buildRes = await request.get(`${BASE}/api/prompt-templates/roles/Senior%20Coder/build`);
        expect(buildRes.status()).toBe(200);
        const buildBody = await buildRes.json();
        expect(buildBody).toHaveProperty('role');
        expect(buildBody).toHaveProperty('prompt');
        expect(buildBody.prompt.length).toBeGreaterThan(0);
      } finally {
        // Clean up assignment
        await request.delete(`${BASE}/api/prompt-templates/assignments/${assignmentId}`);
      }
    } finally {
      // Clean up template
      await request.delete(`${BASE}/api/prompt-templates/${templateId}`);
    }
  });

  test('list assignments for role returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/prompt-templates/roles/Senior%20Coder/assignments`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });
});
