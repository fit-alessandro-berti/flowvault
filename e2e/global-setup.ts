import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync } from 'node:fs';
import { join } from 'node:path';
import { fixtureRoot } from './fixtures';

export default function globalSetup(): void {
  mkdirSync(fixtureRoot, { recursive: true });
  for (const scenario of ['inventory', 'manufacturing'] as const) {
    const run = join(fixtureRoot, `${scenario}-smoke`);
    if (existsSync(join(run, 'manifest.json'))) {
      try {
        execFileSync('python', ['-m', 'saocpm_eval', 'validate', run], {
          cwd: process.cwd(),
          stdio: 'inherit',
        });
        continue;
      } catch {
        rmSync(run, { recursive: true, force: true });
      }
    }
    execFileSync(
      'python',
      [
        '-m',
        'saocpm_eval',
        'generate',
        scenario,
        '--config',
        join(process.cwd(), 'configs', `${scenario}_smoke.yaml`),
        '--out',
        run,
      ],
      { cwd: process.cwd(), stdio: 'inherit' },
    );
    execFileSync('python', ['-m', 'saocpm_eval', 'validate', run], {
      cwd: process.cwd(),
      stdio: 'inherit',
    });
  }
}
