import { spawnSync } from 'node:child_process';

const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const browserChecks = ['qualify:ui-overlays', 'qualify:ui-host-projections'];

for (const script of browserChecks) {
  const result = spawnSync(npmCommand, ['run', script], { stdio: 'inherit' });
  if (result.error) {
    throw new Error(`Unable to run ${script}: ${result.error.message}`);
  }
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
}

console.log(`UI browser qualification passed (${browserChecks.join(', ')}).`);
