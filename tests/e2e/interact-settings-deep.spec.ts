import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe.serial('Deep Settings interaction on Ironweave project', () => {
  test('Save general settings', async ({ page }) => {
    await page.goto('/#/settings/general');

    // Wait for the settings page to load
    const heading = page.locator('h2', { hasText: /^General$/ });
    await expect(heading).toBeVisible({ timeout: 10000 });

    // Get the idle unmount minutes input
    const idleInput = page.locator('#idle-minutes');
    await expect(idleInput).toBeVisible({ timeout: 5000 });

    // Save the original value
    const originalValue = await idleInput.inputValue();

    // Change the value to something different
    const newValue = originalValue === '45' ? '60' : '45';
    await idleInput.fill(newValue);

    // Click Save
    const saveButton = page.locator('button', { hasText: /^Save$/ });
    await expect(saveButton).toBeVisible({ timeout: 5000 });
    await saveButton.click();

    // Verify success message appears
    const successMsg = page.locator('text=Settings saved.');
    await expect(successMsg).toBeVisible({ timeout: 10000 });

    // Restore original value
    await idleInput.fill(originalValue || '30');
    await saveButton.click();
    await page.waitForTimeout(1000);
  });

  test('Settings API CRUD', async ({ request }) => {
    // GET all settings
    const listRes = await request.get(`${BASE}/api/settings`);
    expect(listRes.ok()).toBeTruthy();
    const settingsList = await listRes.json();
    expect(Array.isArray(settingsList)).toBeTruthy();

    // Find the idle_unmount_minutes setting
    const idleSetting = settingsList.find((s: any) => s.key === 'idle_unmount_minutes');
    if (!idleSetting) {
      // If it doesn't exist yet, create it and clean up
      const createRes = await request.put(`${BASE}/api/settings/idle_unmount_minutes`, {
        data: { value: '30', category: 'general' },
      });
      expect(createRes.ok()).toBeTruthy();
      return;
    }

    const originalValue = idleSetting.value;

    // PUT to update the setting
    const updateRes = await request.put(`${BASE}/api/settings/idle_unmount_minutes`, {
      data: { value: '99', category: 'general' },
    });
    expect(updateRes.ok()).toBeTruthy();

    // GET to verify update
    const verifyRes = await request.get(`${BASE}/api/settings`);
    expect(verifyRes.ok()).toBeTruthy();
    const updatedList = await verifyRes.json();
    const updated = updatedList.find((s: any) => s.key === 'idle_unmount_minutes');
    expect(updated).toBeTruthy();
    expect(updated.value).toBe('99');

    // Restore original value
    const restoreRes = await request.put(`${BASE}/api/settings/idle_unmount_minutes`, {
      data: { value: originalValue, category: 'general' },
    });
    expect(restoreRes.ok()).toBeTruthy();
  });

  test('Proxy configs API', async ({ request }) => {
    const res = await request.get(`${BASE}/api/proxy-configs`);
    expect(res.ok()).toBeTruthy();
    const configs = await res.json();
    expect(Array.isArray(configs)).toBeTruthy();
  });
});
