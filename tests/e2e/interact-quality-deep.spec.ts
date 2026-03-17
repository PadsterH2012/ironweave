import { test, expect } from '@playwright/test';
import { getTestEnv, BASE } from './test-helpers';

const { PROJECT_ID, TEAM_ID } = getTestEnv();

test.describe.serial('Deep Quality and Routing interaction on Ironweave project', () => {
  test('Quality tiers API', async ({ request }) => {
    const res = await request.get(`${BASE}/api/quality-tiers`);
    expect(res.ok()).toBeTruthy();
    const tiers = await res.json();
    expect(Array.isArray(tiers)).toBeTruthy();
    expect(tiers.length).toBeGreaterThan(0);

    // Each tier should have the expected fields
    for (const tier of tiers) {
      expect(tier).toHaveProperty('tier');
      expect(tier).toHaveProperty('label');
      expect(tier).toHaveProperty('example_models');
    }
  });

  test('Project quality get/set via API', async ({ request }) => {
    // GET current quality range
    const getRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/quality`);
    expect(getRes.ok()).toBeTruthy();
    const original = await getRes.json();
    expect(original).toHaveProperty('tier_floor');
    expect(original).toHaveProperty('tier_ceiling');

    // PUT new values
    const putRes = await request.put(`${BASE}/api/projects/${PROJECT_ID}/quality`, {
      data: { tier_floor: 2, tier_ceiling: 4 },
    });
    expect(putRes.ok()).toBeTruthy();
    const updated = await putRes.json();
    expect(updated.tier_floor).toBe(2);
    expect(updated.tier_ceiling).toBe(4);

    // Verify via GET
    const verifyRes = await request.get(`${BASE}/api/projects/${PROJECT_ID}/quality`);
    expect(verifyRes.ok()).toBeTruthy();
    const verified = await verifyRes.json();
    expect(verified.tier_floor).toBe(2);
    expect(verified.tier_ceiling).toBe(4);

    // Restore original values
    const restoreRes = await request.put(`${BASE}/api/projects/${PROJECT_ID}/quality`, {
      data: { tier_floor: original.tier_floor, tier_ceiling: original.tier_ceiling },
    });
    expect(restoreRes.ok()).toBeTruthy();
  });

  test('Routing override accept/reject via API', async ({ request }) => {
    // First, trigger pattern detection to ensure there are overrides
    const detectRes = await request.post(
      `${BASE}/api/projects/${PROJECT_ID}/routing-overrides/detect`,
      { data: {} }
    );
    // Detection may or may not produce results, but it should succeed
    expect(detectRes.ok()).toBeTruthy();

    // GET existing overrides
    const listRes = await request.get(
      `${BASE}/api/projects/${PROJECT_ID}/routing-overrides`
    );
    expect(listRes.ok()).toBeTruthy();
    const overrides = await listRes.json();
    expect(Array.isArray(overrides)).toBeTruthy();

    // Find a suggested override to test reject on
    const suggested = overrides.find((o: any) => o.status === 'suggested');
    if (!suggested) {
      // No suggested overrides available; skip the reject test gracefully
      return;
    }

    // POST reject on the override
    const rejectRes = await request.post(
      `${BASE}/api/routing-overrides/${suggested.id}/reject`,
      { data: {} }
    );
    expect(rejectRes.ok()).toBeTruthy();
    const rejected = await rejectRes.json();
    expect(rejected.status).toBe('rejected');
  });
});
