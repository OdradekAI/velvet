import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of ['Document', 'Element', 'Event', 'HTMLElement', 'KeyboardEvent', 'Node', 'MutationObserver']) {
  if (browser[name]) globalThis[name] = browser[name];
}
globalThis.window = browser;
globalThis.document = browser.document;
globalThis.getComputedStyle = browser.getComputedStyle.bind(browser);
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { COMPONENT_META } = await import('/tmp/velvet-plugin-tests/app/ui/component-meta.js');
const { createUiKitStoryHostAdapter, runUiKitStoryScenario, UI_KIT_STORY_REGISTRY } = await import('/tmp/velvet-plugin-tests/app/ui/story-registry.js');

const catalog = JSON.parse(await readFile(new URL('../src/app/ui/ui-kit-catalog.json', import.meta.url), 'utf8'));
assert.equal(catalog.schema, 'velvet.ui.catalog');
assert.equal(catalog.schemaVersion, 1);
assert.deepEqual(catalog.components.map((component) => component.id), COMPONENT_META.map((component) => component.id));

const rootElement = document.createElement('div');
document.body.append(rootElement);
const stories = UI_KIT_STORY_REGISTRY.list();
const browserOnlyComponents = new Set(COMPONENT_META.filter((component) => component.qualification === 'browser').map((component) => component.id));
assert.ok(stories.length >= 40);
assert.ok(stories.filter((story) => story.scenario).length >= 8);
assert.deepEqual(
  stories
    .map((story) => story.scenario?.asyncLifecycle)
    .filter(Boolean)
    .sort(),
  ['cancelled', 'failure', 'retry', 'success', 'timeout'],
);

for (const component of COMPONENT_META) {
  for (const storyName of component.stories) {
    assert.ok(UI_KIT_STORY_REGISTRY.get(`${component.id}/${storyName}`), `${component.id}/${storyName} should be registered`);
  }
}

for (const story of stories) {
  // Browser-qualified components require a real browser EventTarget for portal,
  // focus and dismissable-layer effects; the unified browser gate executes them.
  if (browserOnlyComponents.has(story.componentId)) {
    continue;
  }
  const storyHost = document.createElement('div');
  rootElement.append(storyHost);
  const root = createRoot(storyHost);
  await act(async () => {
    root.render(React.createElement(React.Fragment, null, story.render()));
  });
  assert.equal(typeof story.render, 'function');
  if (story.scenario) {
    const host = createUiKitStoryHostAdapter();
    await runUiKitStoryScenario(story.scenario, {
      root: storyHost,
      act: async (work) => act(work),
      host,
    });
  }
  await act(async () => root.unmount());
  storyHost.remove();
}

console.log(`UI Kit story registry tests passed (${stories.length} stories).`);
