import { createReadStream, existsSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import { extname, join, normalize } from 'node:path';

const root = join(process.cwd(), 'dist', 'flowvault', 'browser');
const contentTypes = {
  '.css': 'text/css; charset=utf-8',
  '.html': 'text/html; charset=utf-8',
  '.ico': 'image/x-icon',
  '.js': 'text/javascript; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.wasm': 'application/wasm',
};

createServer((request, response) => {
  const requested = normalize(
    decodeURIComponent(new URL(request.url ?? '/', 'http://localhost').pathname),
  );
  const candidate = join(root, requested === '/' ? 'index.html' : requested);
  const path =
    candidate.startsWith(root) && existsSync(candidate) && statSync(candidate).isFile()
      ? candidate
      : join(root, 'index.html');
  response.setHeader('Content-Type', contentTypes[extname(path)] ?? 'application/octet-stream');
  createReadStream(path).pipe(response);
}).listen(4200, '127.0.0.1', () => {
  process.stdout.write('FLOWVAULT production server listening on http://127.0.0.1:4200\n');
});
