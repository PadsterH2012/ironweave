import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('API Health', () => {
  test('health endpoint returns ok', async ({ request }) => {
    const response = await request.get(`${BASE}/api/health`);
    expect(response.status()).toBe(200);
    expect(await response.text()).toBe('ok');
  });

  test('projects list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('dashboard stats returns expected shape', async ({ request }) => {
    const response = await request.get(`${BASE}/api/dashboard`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(body).toHaveProperty('project_count');
    expect(body).toHaveProperty('active_agents');
    expect(body).toHaveProperty('open_issues');
  });

  test('settings list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/settings`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('runtimes list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/runtimes`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('agents list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/agents`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('mounts list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/mounts`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('quality tiers returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/quality-tiers`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('roles list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/roles`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('dispatch status returns expected shape', async ({ request }) => {
    const response = await request.get(`${BASE}/api/dispatch/status`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(body).toHaveProperty('paused');
  });
});
