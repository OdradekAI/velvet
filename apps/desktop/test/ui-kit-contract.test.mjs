import assert from 'node:assert/strict';

const {
  HOST_ONLY_UI_SURFACES,
  isAllowedUiEntrypoint,
  PLUGIN_UI_COMPONENTS,
  SUPPORTED_UI_COMPONENT_CONTRACT_VERSION,
  UI_THEME_SCHEMA_VERSION,
  negotiateUiComponentVersion,
  normalizeUiThemeSnapshot,
} = await import('/tmp/velvet-plugin-tests/workbench/plugins/ui-kit.js');
const { PluginUiRegistry } = await import('/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js');

const theme = normalizeUiThemeSnapshot({
  scheme: 'dark',
  revision: 7,
  density: 'compact',
  contrast: 'normal',
  reducedMotion: true,
  tokens: {
    'surface.canvas': '#17181a',
    'text.primary': '#f2f4f7',
    'unknown.token': '#fff',
    'text.secondary': 'url(https://attacker.invalid)',
  },
});

assert.equal(theme.schemaVersion, UI_THEME_SCHEMA_VERSION);
assert.equal(theme.revision, 7);
assert.equal(theme.density, 'compact');
assert.equal(theme.reducedMotion, true);
assert.equal(theme.tokens['surface.canvas'], '#17181a');
assert.equal(theme.tokens['unknown.token'], undefined);
assert.equal(theme.tokens['text.secondary'], undefined);

const legacy = normalizeUiThemeSnapshot({ scheme: 'dark', revision: 0, tokens: {} });
assert.equal(legacy.schemaVersion, UI_THEME_SCHEMA_VERSION);
assert.equal(legacy.density, 'default');
assert.equal(legacy.contrast, 'normal');
assert.equal(legacy.reducedMotion, false);

assert.equal(
  negotiateUiComponentVersion([SUPPORTED_UI_COMPONENT_CONTRACT_VERSION + 1, SUPPORTED_UI_COMPONENT_CONTRACT_VERSION]),
  SUPPORTED_UI_COMPONENT_CONTRACT_VERSION,
);
assert.equal(negotiateUiComponentVersion([SUPPORTED_UI_COMPONENT_CONTRACT_VERSION + 1]), undefined);
assert.ok(HOST_ONLY_UI_SURFACES.includes('preview-host'));
assert.ok(HOST_ONLY_UI_SURFACES.includes('operations-panel'));
assert.ok(PLUGIN_UI_COMPONENTS.includes('button'));
assert.ok(!PLUGIN_UI_COMPONENTS.includes('preview-host'));
assert.equal(isAllowedUiEntrypoint('https://plugin.example/view'), true);
assert.equal(isAllowedUiEntrypoint('wasm://sample/view'), true);
assert.equal(isAllowedUiEntrypoint('https:malformed'), false);
assert.equal(isAllowedUiEntrypoint('javascript:alert(1)'), false);

assert.throws(() => normalizeUiThemeSnapshot({ scheme: 'neon', revision: 0, tokens: {} }));

let endpoint;
const hostMessages = [];
const ui = new PluginUiRegistry();
ui.register({
  id: 'sample.view',
  extensionId: 'sample.extension',
  namespace: 'workbench.app.sample',
  kind: 'view',
  entrypoint: 'https://plugin.example/view',
  bridgeVersion: '1',
  appId: 'sample',
});
ui.bindAdapter('sample.extension', 'sample.view', {
  mount: (nextEndpoint) => {
    endpoint = nextEndpoint;
    nextEndpoint.subscribeHost((message) => hostMessages.push(message));
  },
  unmount: () => undefined,
});
const session = ui.createSession('sample.extension', 'sample.view');
await session.mount({
  context: {
    revision: 0,
    activeAppId: 'sample',
    dataScope: null,
    selection: [],
    query: '',
    filters: {},
    sort: [],
    handoff: null,
  },
  theme: { scheme: 'dark', revision: 0, tokens: {} },
  size: { width: 10, height: 10 },
});
assert.equal(hostMessages[0].componentContractVersion, SUPPORTED_UI_COMPONENT_CONTRACT_VERSION);
endpoint.receive({ type: 'surface.ready', bridgeVersion: '1', componentContractVersion: 99 });
await Promise.resolve();
assert.equal(session.state, 'failed');

console.log('UI Kit contract tests passed.');
