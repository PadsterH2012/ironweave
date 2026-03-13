import { test, expect } from '@playwright/test';
import { login } from './helpers';

test.describe('Projects', () => {
  test.beforeEach(async ({ page }) => {
    await login(page);
  });

  test('creates a new project and verifies it appears in the list', async ({ page }) => {
    await page.goto('/projects');
    await page.click('[data-testid="create-project"]');

    await page.fill('[data-testid="project-name"]', 'Test Project');
    await page.fill('[data-testid="project-description"]', 'A project created by E2E tests');
    await page.click('[data-testid="submit-project"]');

    await expect(page.getByText('Test Project')).toBeVisible();
  });

  test('deletes a project and verifies it is removed', async ({ page }) => {
    await page.goto('/projects');

    // Assume "Test Project" exists from a previous test or seeded data
    const projectRow = page.getByText('Test Project').locator('..');
    await projectRow.getByRole('button', { name: /delete/i }).click();

    // Confirm deletion dialog
    await page.click('[data-testid="confirm-delete"]');

    await expect(page.getByText('Test Project')).not.toBeVisible();
  });
});
