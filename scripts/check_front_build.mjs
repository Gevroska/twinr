import { readFileSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

const dist = fileURLToPath(new URL('../front/dist/', import.meta.url));
const manifest = JSON.parse(readFileSync(resolve(dist, '.vite/manifest.json'), 'utf8'));
const seen = new Set();
function visit(key) {
  if (seen.has(key)) return;
  seen.add(key);
  for (const dependency of manifest[key].imports || []) visit(dependency);
}
for (const [key, chunk] of Object.entries(manifest)) if (chunk.isEntry) visit(key);
const files = [...seen].map(key => manifest[key].file);
if (files.some(file => /\/(hls|Stream|Vod|Clips|Favorites|axios)-/.test(file))) {
  throw new Error('Home page eagerly loads a deferred feature');
}
const bytes = files.reduce((sum, file) => sum + statSync(resolve(dist, file)).size, 0);
if (bytes > 64 * 1024) throw new Error(`Initial JavaScript exceeds 64 KiB: ${bytes}`);
console.log(`Initial JavaScript: ${bytes} bytes; player and secondary routes stay deferred.`);
