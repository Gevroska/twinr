import { mkdirSync, copyFileSync, existsSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

const frontDist = join(process.cwd(), 'front', 'dist');
const outPath = join(process.cwd(), 'public');
const outAssets = join(outPath, 'assets');
const distAssets = join(frontDist, 'assets');
const frontAssets = join(process.cwd(), 'front', 'assets');

if (!existsSync(outAssets)) mkdirSync(outAssets, { recursive: true });

for (const file of readdirSync(distAssets)) copyFileSync(join(distAssets, file), join(outAssets, file));
for (const file of readdirSync(frontAssets)) copyFileSync(join(frontAssets, file), join(outAssets, file));
copyFileSync(join(frontDist, 'index.html'), join(outPath, 'index.html'));

console.log('Frontend assets synchronized to public/.');
