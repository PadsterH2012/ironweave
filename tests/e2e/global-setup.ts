import { request } from '@playwright/test';

const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';

async function globalSetup() {
  const ctx = await request.newContext({ ignoreHTTPSErrors: true });

  // Create test project
  const projectRes = await ctx.post(`${BASE}/api/projects`, {
    data: {
      name: `E2E Test Project ${Date.now()}`,
      directory: '/tmp/e2e-test-project',
      context: 'homelab',
    },
  });
  const project = await projectRes.json();

  // Create a team with common slots
  const teamRes = await ctx.post(`${BASE}/api/projects/${project.id}/teams`, {
    data: {
      name: 'E2E Test Team',
      project_id: project.id,
      coordination_mode: 'swarm',
    },
  });
  const team = await teamRes.json();

  // Create 2 test issues (one open, one closed)
  await ctx.post(`${BASE}/api/projects/${project.id}/issues`, {
    data: {
      project_id: project.id,
      title: 'E2E Open Issue',
      description: 'Test issue for e2e — open',
      status: 'open',
    },
  });

  await ctx.post(`${BASE}/api/projects/${project.id}/issues`, {
    data: {
      project_id: project.id,
      title: 'E2E Closed Issue',
      description: 'Test issue for e2e — closed',
      status: 'closed',
    },
  });

  // Create a test feature with tasks
  const featureRes = await ctx.post(`${BASE}/api/projects/${project.id}/features`, {
    data: {
      project_id: project.id,
      title: 'E2E Test Feature',
      description: 'A test feature seeded by global setup',
      status: 'idea',
      priority: 5,
      prd_content: '## E2E Test PRD\n\n- Validate project setup\n- Verify teardown',
      keywords: ['e2e', 'test'],
    },
  });
  const feature = await featureRes.json();

  if (feature.id) {
    // Add tasks to the feature
    for (const title of ['Setup test data', 'Run assertions', 'Cleanup']) {
      await ctx.post(`${BASE}/api/features/${feature.id}/tasks`, {
        data: { feature_id: feature.id, title, sort_order: 1 },
      });
    }
  }

  // Write IDs to env file that specs can read
  const fs = require('fs');
  const path = require('path');
  const envFile = path.join(__dirname, '.test-env.json');
  fs.writeFileSync(envFile, JSON.stringify({
    PROJECT_ID: project.id,
    TEAM_ID: team.id,
  }));

  // Also set as env vars for the process
  process.env.TEST_PROJECT_ID = project.id;
  process.env.TEST_TEAM_ID = team.id;

  await ctx.dispose();
}

export default globalSetup;
