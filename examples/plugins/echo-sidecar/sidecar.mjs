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
