import { test, expect } from '@playwright/test';

const uniqueName = `E2E Test Project ${Date.now()}`;

test.describe.serial('Project CRUD', () => {
  test('create a new project', async ({ page }) => {
    await page.goto('/#/projects');

    const createButton = page.locator('button', { hasText: /new project|create project|create/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Fill in project name
    const nameInput = page.locator('input[placeholder*="name" i], input[placeholder*="project" i]').first();
    await expect(nameInput).toBeVisible({ timeout: 5000 });
    await nameInput.fill(uniqueName);

    // Fill in directory
    const dirInput = page.locator('input[placeholder*="dir" i], input[placeholder*="path" i]').first();
    if (await dirInput.isVisible({ timeout: 3000 }).catch(() => false)) {
      await dirInput.fill('/tmp/e2e-test');
    }

    // Select context if dropdown exists
    const contextSelect = page.locator('select, [role="listbox"]').first();
    if (await contextSelect.isVisible({ timeout: 2000 }).catch(() => false)) {
      await contextSelect.selectOption({ label: /homelab/i }).catch(() => {});
    }

    // Submit the form
    const submitButton = page.locator('button', { hasText: /create|save|submit/i });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Verify project appears in the list
    await page.goto('/#/projects');
    const projectTile = page.locator(`text=${uniqueName}`);
    await expect(projectTile.first()).toBeVisible({ timeout: 10000 });
  });

  test('edit project settings', async ({ page }) => {
    await page.goto('/#/projects');

    // Click on the created project tile
    const tile = page.locator(`text=${uniqueName}`);
    await expect(tile.first()).toBeVisible({ timeout: 10000 });
    await tile.first().click();
    await page.waitForURL(/\/#\/projects\/.+/, { timeout: 10000 });

    // Navigate to Settings tab
    const settingsTab = page.locator('button', { hasText: /^Settings$/ });
    await expect(settingsTab).toBeVisible({ timeout: 10000 });
    await settingsTab.click();

    // Find and update the description field
    const descInput = page.locator('textarea, input[placeholder*="desc" i]').first();
    await expect(descInput).toBeVisible({ timeout: 5000 });
    await descInput.fill('Updated by E2E test');

    // Save
    const saveButton = page.locator('button', { hasText: /save|update|apply/i });
    await expect(saveButton.first()).toBeVisible({ timeout: 5000 });
    await saveButton.first().click();

    // Reload and verify the description persisted
    await page.reload();
    const settingsTabAgain = page.locator('button', { hasText: /^Settings$/ });
    await expect(settingsTabAgain).toBeVisible({ timeout: 10000 });
    await settingsTabAgain.click();

    const updatedDesc = page.locator('textarea, input[placeholder*="desc" i]').first();
    await expect(updatedDesc).toBeVisible({ timeout: 5000 });
    await expect(updatedDesc).toHaveValue('Updated by E2E test', { timeout: 5000 });
  });

  test('delete the test project', async ({ page }) => {
    await page.goto('/#/projects');

    // Find the test project tile and its delete button
    const tile = page.locator(`text=${uniqueName}`);
    await expect(tile.first()).toBeVisible({ timeout: 10000 });

    // Click the × close/delete button on the tile
    const tileContainer = tile.first().locator('..').locator('..');
    const deleteButton = tileContainer.locator('button', { hasText: /×|✕|delete|remove/i }).first();

    if (await deleteButton.isVisible({ timeout: 3000 }).catch(() => false)) {
      await deleteButton.click();
    } else {
      // Try a generic close button near the tile
      const closeButton = tileContainer.locator('button').filter({ hasText: '×' }).first();
      await closeButton.click();
    }

    // Confirm deletion if a dialog appears
    const confirmButton = page.locator('button', { hasText: /confirm|yes|delete|ok/i });
    if (await confirmButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await confirmButton.first().click();
    }

    // Verify project is removed
    await expect(page.locator(`text=${uniqueName}`)).toHaveCount(0, { timeout: 10000 });
  });
});
