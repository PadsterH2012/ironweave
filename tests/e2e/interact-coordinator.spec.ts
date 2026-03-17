import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

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

test.describe.serial('Coordinator wake/sleep', () => {
  test('Coordinator tab renders', async ({ page }) => {
    await goToProjectTab(page, 'Coordinator');

    // Verify the Coordinator heading loads
    const heading = page.locator('h2', { hasText: 'Coordinator' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Verify state display is visible (shows "Active" or "Dormant")
    const stateLabel = page.locator('.capitalize').first();
    await expect(stateLabel).toBeVisible({ timeout: 10000 });
    const stateText = await stateLabel.textContent();
    expect(['Active', 'Dormant', 'active', 'dormant']).toContain(stateText?.trim());
  });

  test('Wake coordinator', async ({ page }) => {
    await goToProjectTab(page, 'Coordinator');

    // Wait for coordinator panel to load
    const heading = page.locator('h2', { hasText: 'Coordinator' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Check current state - if already active, sleep first
    const stateLabel = page.locator('.capitalize').first();
    await expect(stateLabel).toBeVisible({ timeout: 10000 });
    const currentState = await stateLabel.textContent();

    if (currentState?.trim().toLowerCase() === 'active') {
      // Sleep first so we can test wake
      const sleepButton = page.locator('button', { hasText: /^Sleep$/ });
      await expect(sleepButton).toBeVisible({ timeout: 5000 });
      await sleepButton.click();
      // Wait for state to change to dormant
      await expect(page.locator('.capitalize').first()).toHaveText(/dormant/i, { timeout: 10000 });
    }

    // Click Wake button
    const wakeButton = page.locator('button', { hasText: /^Wake$/ });
    await expect(wakeButton).toBeVisible({ timeout: 5000 });
    await wakeButton.click();

    // Verify state changes to active
    await expect(page.locator('.capitalize').first()).toHaveText(/active/i, { timeout: 10000 });
  });

  test('Sleep coordinator', async ({ page }) => {
    await goToProjectTab(page, 'Coordinator');

    // Wait for coordinator panel to load
    const heading = page.locator('h2', { hasText: 'Coordinator' });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Check current state - if dormant, wake first
    const stateLabel = page.locator('.capitalize').first();
    await expect(stateLabel).toBeVisible({ timeout: 10000 });
    const currentState = await stateLabel.textContent();

    if (currentState?.trim().toLowerCase() === 'dormant') {
      // Wake first so we can test sleep
      const wakeButton = page.locator('button', { hasText: /^Wake$/ });
      await expect(wakeButton).toBeVisible({ timeout: 5000 });
      await wakeButton.click();
      await expect(page.locator('.capitalize').first()).toHaveText(/active/i, { timeout: 10000 });
    }

    // Click Sleep button
    const sleepButton = page.locator('button', { hasText: /^Sleep$/ });
    await expect(sleepButton).toBeVisible({ timeout: 5000 });
    await sleepButton.click();

    // Verify state changes to dormant (this also restores to dormant state)
    await expect(page.locator('.capitalize').first()).toHaveText(/dormant/i, { timeout: 10000 });
  });
});
