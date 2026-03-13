import { test, expect } from '@playwright/test';

test.describe('Login', () => {
  test('redirects unauthenticated users to /login', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/);
  });

  test('logs in with valid credentials and redirects to dashboard', async ({ page }) => {
    await page.goto('/login');
    await page.fill('[data-testid="username"]', 'admin');
    await page.fill('[data-testid="password"]', 'admin');
    await page.click('[data-testid="login-button"]');

    await expect(page).toHaveURL(/\/$/);
    await expect(page.getByRole('heading', { name: /dashboard/i })).toBeVisible();
  });
});
