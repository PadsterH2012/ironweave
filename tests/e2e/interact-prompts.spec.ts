import { test, expect } from '@playwright/test';

const PROJECT_ID = '1d91326e-262a-40d0-980e-d727be5e6e66';
const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
const templateName = `E2E Test Template ${Date.now()}`;
let createdTemplateId: string | undefined;

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

test.describe.serial('Prompt template CRUD', () => {
  test('Prompts tab renders', async ({ page }) => {
    await goToProjectTab(page, 'Prompts');

    // The prompt editor area should load with the Templates / Role Assignments toggle
    const templatesButton = page.locator('button', { hasText: /^Templates$/ });
    await expect(templatesButton).toBeVisible({ timeout: 10000 });

    // Verify either template list or empty state is visible
    const templateArea = page.locator('text=Role Templates').or(
      page.locator('text=No templates yet')
    ).or(
      page.locator('text=Select a template to edit')
    );
    await expect(templateArea.first()).toBeVisible({ timeout: 10000 });
  });

  test('Create a prompt template', async ({ page }) => {
    await goToProjectTab(page, 'Prompts');

    // Ensure we are on the Templates section
    const templatesButton = page.locator('button', { hasText: /^Templates$/ });
    await expect(templatesButton).toBeVisible({ timeout: 10000 });
    await templatesButton.click();

    // Click "+ New" next to "Role Templates" heading
    const newButton = page.locator('button', { hasText: '+ New' }).first();
    await expect(newButton).toBeVisible({ timeout: 5000 });
    await newButton.click();

    // Fill in the template name (placeholder="Template name")
    const nameInput = page.locator('input[placeholder="Template name"]');
    await expect(nameInput).toBeVisible({ timeout: 5000 });
    await nameInput.fill(templateName);

    // Fill in the content textarea
    const contentArea = page.locator('textarea');
    await expect(contentArea).toBeVisible({ timeout: 5000 });
    await contentArea.fill('Test prompt content for E2E validation');

    // Click "Create" button to submit
    const createButton = page.locator('button', { hasText: /^Create$/ });
    await expect(createButton).toBeVisible({ timeout: 5000 });
    await createButton.click();

    // Verify the template appears in the list
    await expect(async () => {
      const templateItem = page.locator(`.font-medium:has-text("${templateName}")`);
      await expect(templateItem.first()).toBeVisible();
    }).toPass({ timeout: 10000, intervals: [2000] });
  });

  test('Delete the test template', async ({ request }) => {
    // Find and delete via API for reliability
    const res = await request.get(`${BASE}/api/prompt-templates?project_id=${PROJECT_ID}`);
    if (res.ok()) {
      const templates = await res.json();
      const target = templates.find((t: any) => t.name === templateName);
      if (target) {
        createdTemplateId = target.id;
        await request.delete(`${BASE}/api/prompt-templates/${target.id}`);
      }
    }

    // Fallback: also try listing all templates
    if (!createdTemplateId) {
      const allRes = await request.get(`${BASE}/api/prompt-templates`);
      if (allRes.ok()) {
        const all = await allRes.json();
        const target = all.find((t: any) => t.name === templateName);
        if (target) {
          await request.delete(`${BASE}/api/prompt-templates/${target.id}`);
        }
      }
    }

    // Verify cleanup
    const checkRes = await request.get(`${BASE}/api/prompt-templates`);
    if (checkRes.ok()) {
      const remaining = await checkRes.json();
      expect(remaining.find((t: any) => t.name === templateName)).toBeUndefined();
    }
  });
});
