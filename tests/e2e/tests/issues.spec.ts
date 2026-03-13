import { test, expect } from '@playwright/test';
import { login } from './helpers';

test.describe('Issues', () => {
  let projectSlug: string;

  test.beforeEach(async ({ page, request }) => {
    await login(page);

    // Create a project via the API for issue tests
    const response = await request.post('/api/projects', {
      data: { name: 'Issue Test Project', description: 'Created for issue E2E tests' },
    });
    const project = await response.json();
    projectSlug = project.slug ?? project.id;
  });

  test('creates an issue and verifies it appears in the Open column', async ({ page }) => {
    await page.goto(`/projects/${projectSlug}/issues`);

    await page.click('[data-testid="create-issue"]');
    await page.fill('[data-testid="issue-title"]', 'Test Issue');
    await page.fill('[data-testid="issue-description"]', 'An issue created by E2E tests');
    await page.click('[data-testid="submit-issue"]');

    const openColumn = page.locator('[data-testid="column-open"]');
    await expect(openColumn.getByText('Test Issue')).toBeVisible();
  });

  test('drags an issue to In Progress and verifies status update', async ({ page }) => {
    await page.goto(`/projects/${projectSlug}/issues`);

    // Assume "Test Issue" exists in Open column
    const issueCard = page.locator('[data-testid="column-open"]').getByText('Test Issue');
    const inProgressColumn = page.locator('[data-testid="column-in-progress"]');

    await issueCard.dragTo(inProgressColumn);

    await expect(inProgressColumn.getByText('Test Issue')).toBeVisible();
    await expect(page.locator('[data-testid="column-open"]').getByText('Test Issue')).not.toBeVisible();
  });
});
