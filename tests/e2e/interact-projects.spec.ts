import { test, expect } from '@playwright/test';

const uniqueName = `E2E Test Project ${Date.now()}`;

test.describe.serial('Project CRUD', () => {
  test('create a new project', async ({ page }) => {
    await page.goto('/#/projects');

    // Click "Create Project" button
    const createButton = page.locator('button', { hasText: /Create Project/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Fill in project name (find the Name input in the form)
    const nameInput = page.locator('input[placeholder*="name" i]').first();
    await expect(nameInput).toBeVisible({ timeout: 5000 });
    await nameInput.fill(uniqueName);

    // Fill in directory
    const dirInput = page.locator('input[placeholder*="dir" i], input[placeholder*="path" i]').first();
    if (await dirInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await dirInput.fill('/tmp/e2e-test');
    }

    // Submit the form (look for Create/Save button in the form area)
    const submitButton = page.locator('button', { hasText: /^Create$/ });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Verify project appears in the list
    await expect(async () => {
      await page.goto('/#/projects');
      const projectTile = page.locator(`text=${uniqueName}`);
      await expect(projectTile.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [3000] });
  });

  test('navigate to the test project', async ({ page }) => {
    await page.goto('/#/projects');
    const tile = page.locator(`text=${uniqueName}`);
    await expect(tile.first()).toBeVisible({ timeout: 10000 });
    await tile.first().click();
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });

    // Verify project detail loaded with correct name
    await expect(page.locator(`h1:has-text("${uniqueName}")`)).toBeVisible({ timeout: 10000 });
  });

  test('delete the test project via API', async ({ request }) => {
    // Clean up via API — more reliable than UI delete
    const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
    const projectsRes = await request.get(`${BASE}/api/projects`);
    const projects = await projectsRes.json();
    const testProject = projects.find((p: any) => p.name === uniqueName);
    if (testProject) {
      await request.delete(`${BASE}/api/projects/${testProject.id}`);
    }

    // Verify it's gone
    const afterRes = await request.get(`${BASE}/api/projects`);
    const after = await afterRes.json();
    expect(after.find((p: any) => p.name === uniqueName)).toBeUndefined();
  });
});
