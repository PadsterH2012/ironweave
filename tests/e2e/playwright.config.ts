import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  timeout: 30000,
  retries: 0,
  globalSetup: './global-setup.ts',
  globalTeardown: './global-teardown.ts',
  use: {
    baseURL: process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk',
    ignoreHTTPSErrors: true,
    screenshot: 'only-on-failure',
  },
  reporter: process.env.CI ? 'json' : 'list',
});
