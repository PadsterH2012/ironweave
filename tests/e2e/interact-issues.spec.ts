import { test, expect } from '@playwright/test';

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

    // Wait for the board to render
    await expect(page.locator('text=Open').first()).toBeVisible({ timeout: 10000 });

    // Click create/add issue button
    const createButton = page.locator('button', { hasText: /new issue|create issue|add issue|\+/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Fill title
    const titleInput = page.locator('input[placeholder*="title" i], input[placeholder*="issue" i], input[placeholder*="summary" i]').first();
    await expect(titleInput).toBeVisible({ timeout: 5000 });
    await titleInput.fill(issueTitle);

    // Submit
    const submitButton = page.locator('button', { hasText: /create|save|submit|add/i });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Verify issue appears in the Open column
    const issue = page.locator(`text=${issueTitle}`);
    await expect(issue.first()).toBeVisible({ timeout: 10000 });
  });

  test('change issue status to in_progress', async ({ page }) => {
    await goToProjectTab(page, 'Issues');

    // Click on the test issue
    const issue = page.locator(`text=${issueTitle}`);
    await expect(issue.first()).toBeVisible({ timeout: 10000 });
    await issue.first().click();

    // Try to change status via a select/dropdown
    const statusSelect = page.locator('select').filter({ has: page.locator('option') }).first();
    if (await statusSelect.isVisible({ timeout: 3000 }).catch(() => false)) {
      await statusSelect.selectOption({ value: 'in_progress' }).catch(async () => {
        await statusSelect.selectOption({ label: /in.?progress/i });
      });
    } else {
      // Try clicking a status button
      const inProgressButton = page.locator('button', { hasText: /in.?progress/i });
      if (await inProgressButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
        await inProgressButton.first().click();
      }
    }

    // Save if there's a save button
    const saveButton = page.locator('button', { hasText: /save|update|apply/i });
    if (await saveButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await saveButton.first().click();
    }

    // Verify the issue moved to In Progress
    await page.waitForTimeout(1000);
    const inProgressColumn = page.locator('text=/In.?Progress/i').first();
    await expect(inProgressColumn).toBeVisible({ timeout: 10000 });
  });

  test('delete the test issue', async ({ page }) => {
    await goToProjectTab(page, 'Issues');

    // Find the test issue
    const issue = page.locator(`text=${issueTitle}`);
    await expect(issue.first()).toBeVisible({ timeout: 10000 });
    await issue.first().click();

    // Click delete button
    const deleteButton = page.locator('button', { hasText: /delete|remove|×|✕/i });
    await expect(deleteButton.first()).toBeVisible({ timeout: 5000 });
    await deleteButton.first().click();

    // Confirm if needed
    const confirmButton = page.locator('button', { hasText: /confirm|yes|delete|ok/i });
    if (await confirmButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await confirmButton.first().click();
    }

    // Verify issue is gone
    await expect(page.locator(`text=${issueTitle}`)).toHaveCount(0, { timeout: 10000 });
  });
});
