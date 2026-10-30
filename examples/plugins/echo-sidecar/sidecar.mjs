import readline from 'node:readline';

const protocolVersion = 1;
const input = readline.createInterface({
  input: process.stdin,
  crlfDelay: Infinity,
});

for await (const line of input) {
  let request;
  try {
    request = JSON.parse(line);
  } catch {
    process.stdout.write(`${JSON.stringify({
      protocolVersion,
      ok: false,
      error: 'invalid JSON request',
    })}\n`);
    continue;
  }

  if (request.protocolVersion !== protocolVersion || request.kind !== 'invoke-command') {
    process.stdout.write(`${JSON.stringify({
      protocolVersion,
      ok: false,
      error: 'unsupported runtime request',
    })}\n`);
    continue;
  }

  if (request.commandId === 'echo.crash') {
    process.exit(17);
  }

  if (request.method === 'preview.open') {
    process.stdout.write(`${JSON.stringify({
      protocolVersion,
      ok: true,
      result: {
        sessionId: `${request.extensionId}:${request.commandId}`,
        renderer: {
          kind: 'ui-bridge',
          entrypoint: 'data:text/html,%3Cstrong%3EEcho%20preview%3C%2Fstrong%3E',
          bridgeVersion: '1',
        },
        commands: ['echo.zoom'],
        slots: ['metadata'],
      },
    })}\n`);
    continue;
  }

  if (request.method === 'preview.augment') {
    process.stdout.write(`${JSON.stringify({
      protocolVersion,
      ok: true,
      result: {
        sessionId: `${request.extensionId}:${request.commandId}`,
        renderer: {
          kind: 'ui-bridge',
          entrypoint: 'data:text/html,%3Csmall%3EEcho%20metadata%3C%2Fsmall%3E',
          bridgeVersion: '1',
        },
        commands: ['echo.inspect-metadata', 'echo.analyze'],
        slots: [],
      },
    })}\n`);
    continue;
  }

  if (request.method === 'preview.action') {
    const result = request.commandId === 'echo.analyze'
      ? {
          kind: 'job',
          jobKind: 'echo.metadata-analysis',
          durationMs: 1200,
        }
      : { kind: 'accepted' };
    process.stdout.write(`${JSON.stringify({
      protocolVersion,
      ok: true,
      result,
    })}\n`);
    continue;
  }

  process.stdout.write(`${JSON.stringify({
    protocolVersion,
    ok: true,
    result: {
      extensionId: request.extensionId,
      commandId: request.commandId,
      args: request.args ?? null,
      context: request.context ?? null,
    },
  })}\n`);
}
