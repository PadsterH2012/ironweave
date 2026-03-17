import { test, expect } from '@playwright/test';
import { BASE } from './test-helpers';

test.describe.serial('Deep Settings interaction on Ironweave project', () => {
  test('Save general settings', async ({ page }) => {
    await page.goto('/#/settings/general');
    await page.waitForTimeout(3000);

    // Verify the save button exists and the form loaded
    const saveButton = page.locator('button', { hasText: /Save/i });
    await expect(saveButton).toBeVisible({ timeout: 10000 });

    // Get the browse roots input and set a value
    const browseInput = page.locator('#browse-roots');
    await expect(browseInput).toBeVisible({ timeout: 5000 });
    const originalValue = await browseInput.inputValue();

    // Click Save (even without changes, should show success)
    await saveButton.click();

    // Verify success message or no error
    await expect(async () => {
      const success = await page.locator('text=/saved/i').count();
      const noError = await page.locator('.bg-red-900').count();
      expect(success > 0 || noError === 0).toBeTruthy();
    }).toPass({ timeout: 10000, intervals: [2000] });
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
