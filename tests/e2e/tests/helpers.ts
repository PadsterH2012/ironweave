import { Page } from '@playwright/test';

export async function login(page: Page, username = 'admin', password = 'admin') {
  await page.goto('/');
  await page.fill('[data-testid="username"]', username);
  await page.fill('[data-testid="password"]', password);
  await page.click('[data-testid="login-button"]');
  await page.waitForURL('**/');
}
