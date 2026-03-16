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

    // Click "New Team" button
    const newTeamButton = page.locator('button', { hasText: /^New Team$/ });
    await expect(newTeamButton).toBeVisible({ timeout: 10000 });
    await newTeamButton.click();

    // Fill team name (placeholder is "backend-team")
    const nameInput = page.locator('input[placeholder="backend-team"]');
    await expect(nameInput).toBeVisible({ timeout: 5000 });
    await nameInput.fill(teamName);

    // Select coordination mode "swarm" (select#team-mode)
    const modeSelect = page.locator('select#team-mode');
    await modeSelect.selectOption('swarm');

    // Click "Create" button (green, in the form)
    const createButton = page.locator('button', { hasText: /^Create$/ });
    await expect(createButton.first()).toBeVisible({ timeout: 5000 });
    await createButton.first().click();

    // Verify team appears in the list
    await expect(async () => {
      const team = page.locator(`text=${teamName}`);
      await expect(team.first()).toBeVisible();
    }).toPass({ timeout: 15000, intervals: [2000] });
  });

  test('team is visible with correct mode', async ({ page }) => {
    await goToProjectTab(page, 'Teams');

    // Verify the team exists
    await expect(page.locator(`text=${teamName}`).first()).toBeVisible({ timeout: 10000 });
    // Verify it shows swarm mode
    await expect(page.locator('text=swarm').first()).toBeVisible({ timeout: 5000 });
  });

  test('delete the test team via API', async ({ request }) => {
    // Clean up via API
    const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
    const projectsRes = await request.get(`${BASE}/api/projects`);
    const projects = await projectsRes.json();
    const pid = projects[0]?.id;
    if (!pid) return;

    const teamsRes = await request.get(`${BASE}/api/projects/${pid}/teams`);
    const teams = await teamsRes.json();
    const testTeam = teams.find((t: any) => t.name === teamName);
    if (testTeam) {
      await request.delete(`${BASE}/api/projects/${pid}/teams/${testTeam.id}`);
    }
  });
});
