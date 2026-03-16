import { test, expect } from '@playwright/test';

const teamName = `E2E Test Team ${Date.now()}`;

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

test.describe.serial('Team CRUD on Ironweave project', () => {
  test('create a new team', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    const newTeamButton = page.locator('button', { hasText: /new team|create team|add team/i });
    await expect(newTeamButton.first()).toBeVisible({ timeout: 10000 });
    await newTeamButton.first().click();

    // Fill team name
    const nameInput = page.locator('input[placeholder*="name" i], input[placeholder*="team" i]').first();
    await expect(nameInput).toBeVisible({ timeout: 5000 });
    await nameInput.fill(teamName);

    // Select mode "swarm"
    const modeSelect = page.locator('select').first();
    if (await modeSelect.isVisible({ timeout: 3000 }).catch(() => false)) {
      await modeSelect.selectOption({ value: 'swarm' }).catch(async () => {
        await modeSelect.selectOption({ label: /swarm/i });
      });
    }

    // Submit
    const submitButton = page.locator('button', { hasText: /create|save|submit|add/i });
    await expect(submitButton.first()).toBeVisible({ timeout: 5000 });
    await submitButton.first().click();

    // Verify team appears
    const team = page.locator(`text=${teamName}`);
    await expect(team.first()).toBeVisible({ timeout: 10000 });
  });

  test('add an agent slot to the team', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    // Expand the test team
    const team = page.locator(`text=${teamName}`);
    await expect(team.first()).toBeVisible({ timeout: 10000 });
    await team.first().click();

    // Add a slot
    const addSlotButton = page.locator('button', { hasText: /add slot|add agent|new slot|\+/i });
    await expect(addSlotButton.first()).toBeVisible({ timeout: 5000 });
    await addSlotButton.first().click();

    // Fill role
    const roleInput = page.locator('input[placeholder*="role" i], select').first();
    await expect(roleInput).toBeVisible({ timeout: 5000 });
    const tagName = await roleInput.evaluate((el: HTMLElement) => el.tagName.toLowerCase());
    if (tagName === 'select') {
      await roleInput.selectOption({ label: /senior coder/i }).catch(async () => {
        await roleInput.selectOption({ value: 'senior_coder' });
      });
    } else {
      await roleInput.fill('Senior Coder');
    }

    // Select runtime
    const runtimeSelect = page.locator('select').filter({ has: page.locator('option[value*="claude" i]') }).first();
    if (await runtimeSelect.isVisible({ timeout: 3000 }).catch(() => false)) {
      await runtimeSelect.selectOption({ value: 'claude' }).catch(async () => {
        await runtimeSelect.selectOption({ label: /claude/i });
      });
    }

    // Save the slot
    const saveButton = page.locator('button', { hasText: /save|add|create|confirm/i });
    await expect(saveButton.first()).toBeVisible({ timeout: 5000 });
    await saveButton.first().click();

    // Verify slot appears
    const slot = page.locator('text=/senior coder|Senior Coder/i');
    await expect(slot.first()).toBeVisible({ timeout: 10000 });
  });

  test('delete the test team', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    // Find the test team
    const team = page.locator(`text=${teamName}`);
    await expect(team.first()).toBeVisible({ timeout: 10000 });

    // Click the delete/× button near the team
    const teamContainer = team.first().locator('..').locator('..');
    const deleteButton = teamContainer.locator('button', { hasText: /×|✕|delete|remove/i }).first();

    if (await deleteButton.isVisible({ timeout: 3000 }).catch(() => false)) {
      await deleteButton.click();
    } else {
      // Try clicking × button closest to the team name
      const closestDelete = team.first().locator('..').locator('button').first();
      await closestDelete.click();
    }

    // Confirm if needed
    const confirmButton = page.locator('button', { hasText: /confirm|yes|delete|ok/i });
    if (await confirmButton.first().isVisible({ timeout: 3000 }).catch(() => false)) {
      await confirmButton.first().click();
    }

    // Verify team is removed
    await expect(page.locator(`text=${teamName}`)).toHaveCount(0, { timeout: 10000 });
  });
});
