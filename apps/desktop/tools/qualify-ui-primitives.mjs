import { readFile } from 'node:fs/promises';
import { spawnSync } from 'node:child_process';

const script = await readFile(new URL('./qualify-ui-primitives.ego.js', import.meta.url), 'utf8');
const result = spawnSync('ego-browser', ['nodejs'], { input: script, encoding: 'utf8', stdio: ['pipe', 'inherit', 'inherit'] });
if (result.error) throw new Error(`ego-browser is required for primitive qualification: ${result.error.message}`);
if (result.status !== 0) process.exit(result.status ?? 1);
