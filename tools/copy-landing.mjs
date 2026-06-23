import { copyFileSync, mkdirSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const source = resolve(root, 'src/landing/index.html');
const target = resolve(root, 'dist/flowvault/index.html');

mkdirSync(dirname(target), { recursive: true });
copyFileSync(source, target);

console.log(`Copied Flowvault landing page to ${target}`);
