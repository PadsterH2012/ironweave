import { request } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

async function globalTeardown() {
  const fs = require('fs');
  const path = require('path');
  const envFile = path.join(__dirname, '.test-env.json');

  if (!fs.existsSync(envFile)) return;

  const env = JSON.parse(fs.readFileSync(envFile, 'utf-8'));
  const ctx = await request.newContext({ ignoreHTTPSErrors: true });

  // Delete team
  if (env.TEAM_ID) {
    await ctx.delete(`${BASE}/api/projects/${env.PROJECT_ID}/teams/${env.TEAM_ID}`).catch(() => {});
  }

  // Delete project
  if (env.PROJECT_ID) {
    await ctx.delete(`${BASE}/api/projects/${env.PROJECT_ID}`).catch(() => {});
  }

  // Clean up env file
  fs.unlinkSync(envFile);

  await ctx.dispose();
}

export default globalTeardown;
