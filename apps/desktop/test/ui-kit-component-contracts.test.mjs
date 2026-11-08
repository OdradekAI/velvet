import assert from 'node:assert/strict';
import { access } from 'node:fs/promises';

const { COMPONENT_META } = await import('/tmp/velvet-plugin-tests/app/ui/component-meta.js');
const contracts = await import('/tmp/velvet-plugin-tests/app/ui/contracts/index.js');
const { UI_KIT_STORY_REGISTRY } = await import('/tmp/velvet-plugin-tests/app/ui/story-registry.js');

assert.equal(COMPONENT_META.length, 27);
const alignedComponents = new Set(['asset-grid', 'command-palette', 'context-menu', 'inspector', 'preview-host']);

for (const meta of COMPONENT_META) {
  const exportName = `${meta.name}Contract`;
  const contract = contracts[exportName];
  assert.ok(contract, `${exportName} should be exported`);
  assert.equal(contract.schema, 'velvet.ui.component-contract');
  assert.equal(contract.schemaVersion, 1);
  assert.equal(contract.id, meta.id);
  assert.equal(contract.layer, meta.layer);
  assert.equal(contract.qualification ?? 'node', meta.qualification ?? 'node');
  assert.equal(contract.documentation, meta.documentation);
  assert.deepEqual(contract.stories, meta.stories);
  assert.deepEqual(contract.asyncLifecycle ?? [], meta.asyncLifecycle ?? []);
  assert.deepEqual(contract.browserActions ?? [], meta.browserActions ?? []);
  if (alignedComponents.has(meta.id)) {
    assert.deepEqual(contract.visual, meta.visual, `${meta.id} visual contract drifted from catalog metadata`);
    assert.deepEqual(contract.behavior, meta.behavior, `${meta.id} behavior contract drifted from catalog metadata`);
    assert.ok(contract.visual?.geometry?.length, `${meta.id} should publish named geometry invariants`);
    assert.equal(contract.behavior?.stateOwner, 'host', `${meta.id} should keep state ownership in the host`);
    const lifecycleActions = new Set((contract.behavior?.actionLifecycle ?? []).map((entry) => entry.action));
    for (const action of meta.actions ?? []) {
      assert.ok(lifecycleActions.has(action), `${meta.id} should define lifecycle transitions for ${action}`);
    }
  }
  const storyActions = new Set(
    UI_KIT_STORY_REGISTRY
      .forComponent(meta.id)
      .flatMap((story) => story.scenario?.covers ?? []),
  );
  const coveredActions = new Set([...storyActions, ...(meta.browserActions ?? [])]);
  assert.deepEqual(
    (meta.actions ?? []).filter((action) => !coveredActions.has(action)),
    [],
    `${meta.id} has uncovered actions`,
  );
  for (const lifecycle of meta.asyncLifecycle ?? []) {
    assert.ok(
      UI_KIT_STORY_REGISTRY.forComponent(meta.id).some((story) => story.scenario?.asyncLifecycle === lifecycle),
      `${meta.id} should have a story for async lifecycle ${lifecycle}`,
    );
  }
  await access(new URL(`../src/app/ui/contracts/${meta.layer}/${meta.id}.contract.ts`, import.meta.url));
  await access(new URL(`../src/app/ui/stories/${meta.layer}/${meta.id}.stories.tsx`, import.meta.url));
  await access(new URL(`../../../${meta.documentation}`, import.meta.url));
}

console.log(`UI Kit component contracts passed (${COMPONENT_META.length} contracts).`);
