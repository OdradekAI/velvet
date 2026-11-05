import { rm, symlink } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const compiledRoot = '/tmp/velvet-plugin-tests';
const dependencyRoot = path.join(compiledRoot, 'node_modules');

await rm(dependencyRoot, { recursive: true, force: true });
await symlink(path.join(desktopRoot, 'node_modules'), dependencyRoot, 'junction');
