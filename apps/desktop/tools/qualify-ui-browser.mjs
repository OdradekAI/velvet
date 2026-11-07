import { spawn, spawnSync } from 'node:child_process';

const npmCommand = process.platform === 'win32' ? 'npm.cmd' : 'npm';
const browserChecks = process.env.UI_BROWSER_ENGINE === 'playwright'
  ? ['playwright']
  : ['qualify:ui-overlays', 'qualify:ui-host-projections', 'qualify:ui-primitives'];
const fixtureUrl = 'http://127.0.0.1:5173/?fixture=eagle';

async function serverReady() {
  try {
    const response = await fetch(fixtureUrl, { signal: AbortSignal.timeout(1000) });
    return response.ok;
  } catch {
    return false;
  }
}

async function waitForServer(processHandle) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (await serverReady()) return;
    if (processHandle.exitCode !== null) {
      throw new Error(`Vite fixture server exited before readiness (code ${processHandle.exitCode ?? 'unknown'})`);
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  throw new Error('Timed out waiting for the Vite fixture server at http://127.0.0.1:5173');
}

let devServer;
if (!(await serverReady())) {
  devServer = spawn(npmCommand, ['run', 'dev', '--', '--host', '127.0.0.1', '--port', '5173'], { stdio: 'inherit' });
  await waitForServer(devServer);
}

let failureCode = 0;
try {
  for (const script of browserChecks) {
    const command = script === 'playwright'
      ? [process.execPath, ['tools/qualify-ui-browser-playwright.mjs']]
      : [npmCommand, ['run', script]];
    const result = spawnSync(command[0], command[1], { stdio: 'inherit' });
    if (result.error) {
      throw new Error(`Unable to run ${script}: ${result.error.message}`);
    }
    if (result.status !== 0) {
      failureCode = result.status ?? 1;
      break;
    }
  }
} finally {
  if (devServer) {
    devServer.kill('SIGTERM');
  }
}

if (failureCode !== 0) process.exit(failureCode);
console.log(`UI browser qualification passed (${browserChecks.join(', ')}).`);
