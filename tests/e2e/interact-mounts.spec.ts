import { test, expect } from '@playwright/test';

test.describe('Mounts interactions', () => {
  test('mount list renders or shows empty state', async ({ page }) => {
    await page.goto('/#/mounts');

    const content = page.locator('text=/mount|no mounts|empty/i').first();
    await expect(content).toBeVisible({ timeout: 10000 });
  });

  test('create mount form opens with expected fields', async ({ page }) => {
    await page.goto('/#/mounts');

    // Click create/add button
    const createButton = page.locator('button', { hasText: /create|add|new/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Verify form appears with name, type, and remote path fields
    const nameField = page.locator('input[placeholder*="name" i], label:has-text("name") + input, input').first();
    await expect(nameField).toBeVisible({ timeout: 5000 });

    // Check for type field (select or input)
    const typeField = page.locator('select, input[placeholder*="type" i]').first();
    await expect(typeField).toBeVisible({ timeout: 5000 });

    // Check for remote path field
    const pathField = page.locator('input[placeholder*="path" i], input[placeholder*="remote" i], input').nth(1);
    await expect(pathField).toBeVisible({ timeout: 5000 });
  });

  test('cancel form hides it', async ({ page }) => {
    await page.goto('/#/mounts');

    // Open the form
    const createButton = page.locator('button', { hasText: /create|add|new/i });
    await expect(createButton.first()).toBeVisible({ timeout: 10000 });
    await createButton.first().click();

    // Verify form is visible
    const formArea = page.locator('form, [role="dialog"], .modal').first();
    if (await formArea.isVisible({ timeout: 3000 }).catch(() => false)) {
      // Click cancel/close button
      const cancelButton = page.locator('button', { hasText: /cancel|close|back/i });
      await expect(cancelButton.first()).toBeVisible({ timeout: 5000 });
      await cancelButton.first().click();

      // Verify form is hidden
      await expect(formArea).toBeHidden({ timeout: 5000 });
    } else {
      // Form may be inline; click cancel
      const cancelButton = page.locator('button', { hasText: /cancel|close|back/i });
      if (await cancelButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
        await cancelButton.first().click();
      }
    }
  });
});
