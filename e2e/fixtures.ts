import { join } from 'node:path';

export const fixtureRoot = '/tmp/flowvault-e2e-fixtures';
export const fixturePath = (scenario: 'inventory' | 'manufacturing'): string =>
  join(fixtureRoot, `${scenario}-smoke`, 'observed.ocel.json');
