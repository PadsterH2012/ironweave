import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

test.describe('Project-scoped API endpoints', () => {
  test('project get returns expected shape', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(body).toHaveProperty('id');
    expect(body).toHaveProperty('name');
    expect(body).toHaveProperty('directory');
  });

  test('issues list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/issues`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('teams list returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/teams`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('merge queue returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/merge-queue`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('loom entries returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/loom`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('cost summary returns 200', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/costs/summary`);
    expect(response.status()).toBe(200);
  });

  test('quality config returns 200', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/quality`);
    expect(response.status()).toBe(200);
  });

  test('coordinator returns 200', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/coordinator`);
    expect(response.status()).toBe(200);
  });

  test('dispatch status returns expected shape', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/dispatch/status`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(body).toHaveProperty('paused');
  });

  test('test runs returns array', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/tests/runs`);
    expect(response.status()).toBe(200);
    const body = await response.json();
    expect(Array.isArray(body)).toBe(true);
  });

  test('swarm status returns 200', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/swarm-status`);
    expect(response.status()).toBe(200);
  });

  test('performance stats returns 200', async ({ request }) => {
    const response = await request.get(`${BASE}/api/projects/${PROJECT_ID}/performance/stats`);
    expect(response.status()).toBe(200);
  });
});
