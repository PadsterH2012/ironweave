import * as fs from 'fs';
import * as path from 'path';

export function getTestEnv(): { PROJECT_ID: string; TEAM_ID: string } {
  const envFile = path.join(__dirname, '.test-env.json');
  if (fs.existsSync(envFile)) {
    return JSON.parse(fs.readFileSync(envFile, 'utf-8'));
  }
  // Fallback for running individual tests
  return {
    PROJECT_ID: process.env.TEST_PROJECT_ID || '1d91326e-262a-40d0-980e-d727be5e6e66',
    TEAM_ID: process.env.TEST_TEAM_ID || '',
  };
}

export const BASE = process.env.BASE_URL || 'https://hl-ironweave-dev.techpad.uk';
