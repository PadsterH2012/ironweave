import { test, expect } from '@playwright/test';
import { BASE } from './test-helpers';

test.describe.serial('Deep Mounts interaction on Ironweave project', () => {
  test('Mount list with status indicators', async ({ page }) => {
    await page.goto('/#/mounts');

    // Wait for mounts page to load
    const heading = page.locator('h1', { hasText: /^Mounts$/ });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Verify mount entries appear (or the empty state message)
    const mountCards = page.locator('.rounded-xl.bg-gray-900');
    await expect(mountCards.first()).toBeVisible({ timeout: 10000 });

    // Each mount card should display a state badge (mounted/unmounted/error)
    const stateBadges = page.locator('.rounded-full:text-matches("mounted|unmounted|error", "i")');
    const badgeCount = await stateBadges.count();
    // If there are mounts, there should be state badges
    if (badgeCount > 0) {
      const firstBadge = stateBadges.first();
      const text = await firstBadge.textContent();
      expect(['mounted', 'unmounted', 'error']).toContain(text?.trim());
    }
  });

  test('Mount status API', async ({ request }) => {
    // GET all mounts
    const listRes = await request.get(`${BASE}/api/mounts`);
    expect(listRes.ok()).toBeTruthy();
    const mounts = await listRes.json();
    expect(Array.isArray(mounts)).toBeTruthy();

    // For each mount, GET its individual status
    for (const mount of mounts) {
      const statusRes = await request.get(`${BASE}/api/mounts/${mount.id}/status`);
      expect(statusRes.ok()).toBeTruthy();
      const status = await statusRes.json();
      expect(status).toBeTruthy();
    }
  });

  test('Mount CRUD structure', async ({ request }) => {
    // GET all mounts and verify field structure
    const listRes = await request.get(`${BASE}/api/mounts`);
    expect(listRes.ok()).toBeTruthy();
    const mounts = await listRes.json();
    expect(Array.isArray(mounts)).toBeTruthy();

    for (const mount of mounts) {
      expect(mount).toHaveProperty('id');
      expect(mount).toHaveProperty('name');
      expect(mount).toHaveProperty('mount_type');
      expect(mount).toHaveProperty('state');
    }
  });
});
