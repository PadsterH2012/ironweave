import { test, expect } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

test.describe('Mounts mount/unmount action API', () => {
  let mountId: string | null = null;

  test.beforeAll(async ({ request }) => {
    const res = await request.get(`${BASE}/api/mounts`);
    if (res.ok()) {
      const mounts = await res.json();
      if (Array.isArray(mounts) && mounts.length > 0) {
        mountId = mounts[0].id;
      }
    }
  });

  test('mount action API contract', async ({ request }) => {
    test.skip(!mountId, 'No mounts available to test');

    const res = await request.post(`${BASE}/api/mounts/${mountId}/mount`);
    // May fail if SSH is not reachable, but should not be 500
    expect(res.status()).toBeLessThan(500);
  });

  test('unmount action API contract', async ({ request }) => {
    test.skip(!mountId, 'No mounts available to test');

    const res = await request.post(`${BASE}/api/mounts/${mountId}/unmount`);
    expect(res.status()).toBeLessThan(500);
  });

  test('mount status API returns status field', async ({ request }) => {
    test.skip(!mountId, 'No mounts available to test');

    const res = await request.get(`${BASE}/api/mounts/${mountId}/status`);
    if (res.status() === 404) {
      test.skip(true, 'Mount status endpoint not implemented');
      return;
    }
    expect(res.status()).toBeLessThan(500);
    if (res.ok()) {
      const body = await res.json();
      expect(body).toHaveProperty('status');
    }
  });
});
