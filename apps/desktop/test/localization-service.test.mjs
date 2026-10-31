import assert from 'node:assert/strict';
import { LocalizationService } from '/tmp/velvet-plugin-tests/workbench/localization/localization-service.js';
import { PluginUiRegistry } from '/tmp/velvet-plugin-tests/workbench/plugins/ui-bridge.js';
import { PluginClient } from '/tmp/velvet-plugin-tests/workbench/plugins/plugin-client.js';

const manifest = {
  defaultLocale: 'en',
  supportedLocales: ['en', 'zh-Hans', 'fr'],
  resourceRoot: 'l10n',
  bundleSchema: 1,
};

const service = new LocalizationService('zh-Hans');
service.registerExtension('sample.reader', manifest, [
  {
    locale: 'en',
    messages: {
      greeting: 'Hello, {name}!',
      items: '{count, plural, =0 {No items} one {# item} other {# items}}',
      role: '{gender, select, female {She} male {He} other {They}}',
      count: '{value, number}',
    },
  },
  {
    locale: 'zh-Hans',
    messages: {
      greeting: '你好，{name}！',
      items: '{count, plural, other {共 # 项}}',
      role: '{gender, select, female {她} male {他} other {他们}}',
      count: '{value, number}',
    },
  },
]);

const handle = service.handle('sample.reader');
assert.equal(handle.t('greeting', 'Fallback, {name}!', { name: 'Ada' }), '你好，Ada！');
assert.equal(
  handle.t('items', '{count, plural, =0 {No items} one {# item} other {# items}}', { count: 3 }),
  '共 3 项',
);
assert.equal(
  handle.t('role', '{gender, select, female {She} male {He} other {They}}', { gender: 'female' }),
  '她',
);
assert.match(
  handle.t('count', '{value, number}', { value: 1234 }),
  /1,234|1\s?234|1.234/,
);
assert.equal(handle.t('missing', 'Default {name}', { name: 'Ada' }), 'Default Ada');

const oldRevision = handle.revision;
assert.equal(service.setLocale('fr-CA'), true);
assert.equal(handle.revision, oldRevision + 1);
// No French bundle is supplied, so the service falls back by language, then
// the extension default locale.
assert.equal(handle.t('greeting', 'Fallback, {name}!', { name: 'Ada' }), 'Hello, Ada!');

service.registerExtension('sample.bad', manifest, [
  {
    locale: 'en',
    messages: {
      greeting: 'Broken {different}',
      'sample.bad.greeting': 'Duplicate id',
    },
  },
  {
    locale: 'de',
    messages: { ignored: 'Unsupported' },
  },
]);
const badHandle = service.handle('sample.bad');
assert.equal(
  badHandle.t('greeting', 'Safe {name}', { name: 'Ada' }),
  'Safe Ada',
);
assert.ok(service.getDiagnostics().some((diagnostic) => diagnostic.code === 'invalid-locale'));
assert.ok(service.getDiagnostics().some((diagnostic) => diagnostic.code === 'invalid-message'));

service.registerExtension('sample.malformed', manifest, [{ locale: 'en', messages: null }]);
assert.equal(
  service.t('sample.malformed', 'safe', 'Safe default'),
  'Safe default',
);
assert.ok(service.getDiagnostics().some((diagnostic) => diagnostic.code === 'invalid-bundle'));

const loadResult = await service.loadExtension('sample.loader', manifest, {
  async load() {
    throw new Error('bundle read failed');
  },
});
assert.equal(loadResult, true);
assert.equal(
  service.t('sample.loader', 'greeting', 'Loader fallback'),
  'Loader fallback',
);
assert.equal(service.unregisterExtension('sample.loader'), true);

const ui = new PluginUiRegistry();
ui.register({
  id: 'reader-view',
  extensionId: 'sample.reader',
  namespace: 'workbench',
  kind: 'view',
  entrypoint: 'builtin://reader-view',
  bridgeVersion: '1',
});
const hostMessages = [];
ui.bindAdapter('sample.reader', 'reader-view', {
  mount(endpoint) {
    endpoint.subscribeHost((message) => hostMessages.push(message));
  },
  unmount() {},
});
ui.projectLocale('sample.reader', service.projection('sample.reader'));
const session = ui.createSession('sample.reader', 'reader-view');
await session.mount({
  context: {
    revision: 0,
    activeAppId: 'asset-manager',
    dataScope: null,
    selection: [],
    query: '',
    filters: {},
    sort: [],
    handoff: null,
  },
  theme: { scheme: 'dark', revision: 0, tokens: {} },
  size: { width: 100, height: 100 },
});
assert.equal(hostMessages.find((message) => message.type === 'host.ready').locale.locale, 'en');
service.setLocale('zh-Hans');
ui.projectLocale('sample.reader', service.projection('sample.reader'));
assert.equal(hostMessages.at(-1).type, 'host.locale');
assert.equal(hostMessages.at(-1).locale.locale, 'zh-Hans');
await session.dispose();

const pluginLocalization = new LocalizationService('en');
const pluginClient = new PluginClient(
  { request: async () => ({ kind: 'accepted' }), subscribe: () => () => {} },
  undefined,
  undefined,
  '1',
  undefined,
  undefined,
  undefined,
  undefined,
  undefined,
  pluginLocalization,
);
pluginClient.register({
  id: 'sample.localized',
  version: '1.0.0',
  hostApi: '1',
  localization: manifest,
  contributions: [],
  permissions: {},
});
assert.equal(
  pluginClient.registerLocalization('sample.localized', [
    { locale: 'en', messages: { ready: 'Ready' } },
  ]),
  true,
);
assert.equal(pluginLocalization.t('sample.localized', 'ready', 'Fallback'), 'Ready');
await pluginClient.unregister('sample.localized');
assert.equal(pluginLocalization.projection('sample.localized'), undefined);

let profileState = { schemaVersion: 1, revision: 0, locale: 'en' };
let profileListener;
const profileCore = {
  async request(request) {
    if (request.kind === 'query-profile') {
      return { kind: 'profile', state: profileState };
    }
    assert.equal(request.kind, 'set-locale');
    assert.equal(request.expectedRevision, profileState.revision);
    profileState = {
      schemaVersion: 1,
      revision: profileState.revision + 1,
      locale: request.locale,
    };
    profileListener?.({
      kind: 'profile-changed',
      revision: profileState.revision,
      state: profileState,
    });
    return { kind: 'profile', state: profileState };
  },
  subscribe(listener) {
    profileListener = listener;
    return () => {
      profileListener = undefined;
    };
  },
};
const persistedLocalization = new LocalizationService('en');
persistedLocalization.bindCore(profileCore);
await persistedLocalization.loadFromCore(profileCore);
assert.equal(persistedLocalization.snapshot.locale, 'en');
assert.equal(await persistedLocalization.setLocaleInCore(profileCore, 'zh-Hans'), true);
assert.equal(persistedLocalization.snapshot.locale, 'zh-Hans');
assert.equal(persistedLocalization.snapshot.revision, 1);
persistedLocalization.dispose();

console.log('Localization service tests passed.');
