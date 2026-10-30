import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { symlinkSync } from 'node:fs';
import { Window } from 'happy-dom';

try {
  symlinkSync(
    fileURLToPath(new URL('../node_modules', import.meta.url)),
    '/tmp/velvet-plugin-tests/node_modules',
    'junction',
  );
} catch {
  // The compiled test tree may already point at the desktop dependencies.
}

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document',
  'DOMRect',
  'DOMRectReadOnly',
  'Element',
  'Event',
  'FocusEvent',
  'HTMLElement',
  'InputEvent',
  'KeyboardEvent',
  'MessageEvent',
  'MouseEvent',
  'MutationObserver',
  'Node',
  'PointerEvent',
  'ResizeObserver',
]) {
  if (browser[name]) {
    globalThis[name] = browser[name];
  }
}
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: browser.navigator,
});
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.requestAnimationFrame = browser.requestAnimationFrame.bind(browser);
globalThis.cancelAnimationFrame = browser.cancelAnimationFrame.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { PluginSurfaceHost } = await import('/tmp/velvet-plugin-tests/app/renderer/plugin-surface-host.js');
const { CommandRegistry } = await import('/tmp/velvet-plugin-tests/workbench/commands/command-registry.js');
const { ContextKeyService } = await import('/tmp/velvet-plugin-tests/workbench/commands/context-key-service.js');
const { MenuRegistry } = await import('/tmp/velvet-plugin-tests/workbench/commands/menu-registry.js');
const { OverlayRegistry } = await import('/tmp/velvet-plugin-tests/workbench/overlays/overlay-registry.js');
const { PluginClient } = await import('/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js');
const { PluginUiRegistry } = await import('/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js');
const { ViewRegistry } = await import('/tmp/velvet-plugin-tests/workbench/views/view-registry.js');
const { InMemoryWorkspaceContext } = await import('/tmp/velvet-plugin-tests/workbench/services/workspace-context.js');
const { WorkbenchAppRegistry } = await import('/tmp/velvet-plugin-tests/workbench/apps/workbench-app.js');

const exampleRoot = fileURLToPath(new URL('../../../examples/plugins/echo-sidecar/', import.meta.url));
const manifest = JSON.parse(await readFile(`${exampleRoot}/manifest.json`, 'utf8'));
const sidecarPath = `${exampleRoot}/sidecar.mjs`;

function normalizeContribution(contribution) {
  return {
    ...contribution,
    container: contribution.container ?? null,
    zone: contribution.zone ?? null,
    title: contribution.title ?? null,
    titleKey: contribution.titleKey ?? null,
    icon: contribution.icon ?? null,
    order: contribution.order ?? null,
    defaultVisible: contribution.defaultVisible ?? null,
    viewContainerId: contribution.viewContainerId ?? null,
    activityBar: contribution.activityBar ?? null,
    label: contribution.label ?? null,
    labelKey: contribution.labelKey ?? null,
    commandId: contribution.commandId ?? null,
    group: contribution.group ?? null,
    when: contribution.when ?? null,
    anchor: contribution.anchor ?? null,
    ui: contribution.ui ?? null,
  };
}

const projection = (state = 'active') => ({
  summary: {
    id: manifest.id,
    version: manifest.version,
    active: state === 'active',
    state,
  },
  manifest: {
    ...manifest,
    contributions: manifest.contributions.map(normalizeContribution),
  },
});

function startRuntime() {
  const runtime = spawn(process.execPath, [sidecarPath], {
    stdio: ['pipe', 'pipe', 'pipe'],
  });
  runtime.stderr.resume();
  return runtime;
}

function invokeRuntime(runtime, request) {
  return new Promise((resolve, reject) => {
    let buffer = '';
    let settled = false;
    const finish = (callback, value) => {
      if (settled) {
        return;
      }
      settled = true;
      runtime.stdout.off('data', onData);
      runtime.off('close', onClose);
      callback(value);
    };
    const onData = (chunk) => {
      buffer += chunk.toString();
      const newline = buffer.indexOf('\n');
      if (newline === -1) {
        return;
      }
      const line = buffer.slice(0, newline);
      finish(resolve, JSON.parse(line));
    };
    const onClose = (code, signal) => {
      finish(reject, new Error(`sidecar exited before responding (${code ?? signal})`));
    };
    runtime.stdout.on('data', onData);
    runtime.once('close', onClose);
    try {
      runtime.stdin.write(`${JSON.stringify(request)}\n`);
    } catch (error) {
      finish(reject, error);
    }
  });
}

function stopRuntime(runtime) {
  return new Promise((resolve) => {
    if (runtime.exitCode !== null || runtime.signalCode !== null) {
      resolve();
      return;
    }
    runtime.once('close', resolve);
    runtime.kill();
  });
}

const requests = [];
const eventListeners = new Set();
let runtime = startRuntime();
let runtimeState = 'active';
const core = {
  request: async (request) => {
    requests.push(request);
    if (request.kind === 'query-plugin-projection') {
      return { kind: 'plugin-projection', projection: projection(runtimeState) };
    }
    if (request.kind === 'invoke-plugin-command') {
      try {
        const response = await invokeRuntime(runtime, {
          protocolVersion: 1,
          kind: 'invoke-command',
          extensionId: request.extensionId,
          commandId: request.commandId,
          args: request.args,
          context: request.context,
        });
        if (!response.ok) {
          throw new Error(response.error ?? 'sidecar rejected the command');
        }
        return { kind: 'accepted' };
      } catch (error) {
        runtimeState = 'failed';
        for (const listener of eventListeners) {
          listener({
            kind: 'plugin-disabled',
            extensionId: manifest.id,
            reason: 'crash',
          });
        }
        throw error;
      }
    }
    if (request.kind === 'invoke-preview-action') {
      const response = await invokeRuntime(runtime, {
        protocolVersion: 1,
        kind: 'invoke-command',
        method: 'preview.action',
        extensionId: request.extensionId,
        commandId: request.commandId,
        args: request.args,
        context: request.context,
      });
      if (!response.ok) {
        throw new Error(response.error ?? 'sidecar rejected the preview action');
      }
      if (response.result?.kind === 'job') {
        return {
          kind: 'job',
          job: {
            id: 'job-preview-000001',
            kind: response.result.jobKind,
            status: 'queued',
            progress: 0,
            message: null,
            attempt: 1,
            retryable: true,
            durationMs: response.result.durationMs,
            willFail: false,
            provenance: {
              ownerId: request.extensionId,
              assetId: request.context.assetId,
              contentKind: request.context.contentKind,
              inputFingerprint: null,
              providerId: request.context.providerId,
              providerVersion: null,
              settingsFingerprint: null,
              schemaVersion: 1,
            },
            result: null,
          },
        };
      }
      return { kind: 'accepted' };
    }
    return { kind: 'accepted' };
  },
  subscribe: (listener) => {
    eventListeners.add(listener);
    return () => eventListeners.delete(listener);
  },
};

const views = new ViewRegistry();
const ui = new PluginUiRegistry();
const contextKeys = new ContextKeyService();
const commands = new CommandRegistry(contextKeys);
const menus = new MenuRegistry(commands, contextKeys);
const overlays = new OverlayRegistry(contextKeys);
const apps = new WorkbenchAppRegistry([{
  id: 'asset-manager',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: { trust: 'trusted', entrypoint: 'builtin://resource-manager' },
}]);
const client = new PluginClient(core, undefined, apps, '1', ui, views, commands, menus, overlays);

await act(async () => {
  await client.applyCoreProjection(projection());
});
assert.ok(views.get('velvet.example.echo.echo.view'));
assert.ok(ui.getDescriptor(manifest.id, 'echo.view'));
assert.deepEqual(
  views.listActivityBarEntries('asset-manager').map((entry) => entry.title),
  ['Echo Tools'],
);
assert.equal(menus.list('context-menu', { anchor: 'asset' }).length, 1);

await commands.execute(
  'echo.open',
  { source: 'frontend-e2e' },
  { keys: { resourceKind: 'asset' } },
);
assert.equal(requests.at(-1).kind, 'invoke-plugin-command');
assert.equal(requests.at(-1).commandId, 'echo.open');

const previewActionResponse = await commands.execute(
  'echo.analyze',
  { scope: 'metadata' },
  {
    keys: {},
    preview: {
      sessionId: 'velvet.example.echo:echo.metadata',
      providerId: 'velvet.example.echo.echo.metadata',
      assetId: 'asset-1',
      contentKind: 'text/plain',
      intent: 'quick',
      surface: 'workspace',
    },
  },
);
assert.equal(requests.at(-1).kind, 'invoke-preview-action');
assert.equal(requests.at(-1).context.assetId, 'asset-1');
assert.equal(previewActionResponse.kind, 'job');
assert.equal(previewActionResponse.job.kind, 'echo.metadata-analysis');

const context = new InMemoryWorkspaceContext();
const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
await act(() => {
  root.render(React.createElement(PluginSurfaceHost, {
    context,
    extensionId: manifest.id,
    surfaceId: 'echo.view',
    ui,
  }));
});
await act(async () => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelector('iframe')?.src.startsWith('data:text/html'), true);
assert.equal(rootElement.querySelector('[data-plugin-surface-state="active"]') !== null, true);
assert.equal(ui.getSession(manifest.id, 'echo.view')?.state, 'active');

await act(async () => {
  await assert.rejects(
    () => commands.execute('echo.crash', null, { keys: {} }),
    /sidecar exited before responding/,
  );
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(views.get('velvet.example.echo.echo.view'), undefined);
assert.equal(ui.getDescriptor(manifest.id, 'echo.view'), undefined);
assert.equal(ui.getSession(manifest.id, 'echo.view'), undefined);
assert.equal(rootElement.querySelector('iframe'), null);

await stopRuntime(runtime);
runtime = startRuntime();
runtimeState = 'active';
await act(async () => {
  await client.applyCoreProjection(projection('active'));
});
await act(async () => {
  await new Promise((resolve) => setTimeout(resolve, 0));
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.ok(views.get('velvet.example.echo.echo.view'));
assert.equal(ui.getSession(manifest.id, 'echo.view')?.state, 'active');
await commands.execute('echo.open', null, { keys: {} });
assert.equal(requests.at(-1).commandId, 'echo.open');

await act(() => root.unmount());
await stopRuntime(runtime);
console.log('External plugin end-to-end tests passed.');
