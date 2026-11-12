import assert from 'node:assert/strict';
import test from 'node:test';
import { assertManifestFields, assertReviewedTranslation, assertTranslationContractParity, pageRoute, sourceHash } from './docs-content.mjs';

test('manifest checks reject a translated top-level permissions field and incorrect optionality', () => {
  const rust = 'struct RawExtensionManifest {\n    required_capabilities: Vec<String>,\n    launch: Option<RuntimeLaunchSpec>,\n    extensions: BTreeMap<String, Value>,\n}';
  const correct = '{/* manifest-fields:start */}\n| `requiredCapabilities` | `required` | capabilities |\n| `launch` | `optional` | runtime launch |\n{/* manifest-fields:end */}';
  assert.doesNotThrow(() => assertManifestFields(rust, correct));
  assert.throws(() => assertManifestFields(rust, correct.replace('requiredCapabilities', 'permissions')), /differ/);
  assert.throws(() => assertManifestFields(rust, correct.replace('`optional`', '`required`')), /differ/);
});

test('reviewed translations detect edits on either side; summaries do not masquerade as reviewed', () => {
  const source = 'English contract';
  const translation = '中文契约';
  const record = { status: 'reviewed', sourceHash: sourceHash(source), translationHash: sourceHash(translation) };
  assert.doesNotThrow(() => assertReviewedTranslation('reference/example.md', source, translation, record));
  assert.throws(() => assertReviewedTranslation('reference/example.md', source + ' changed', translation, record), /stale/);
  assert.throws(() => assertReviewedTranslation('reference/example.md', source, translation + '修改', record), /stale/);
  assert.throws(() => assertReviewedTranslation('reference/example.md', source, translation, undefined), /Missing/);
  assert.doesNotThrow(() => assertReviewedTranslation('reference/example.md', source, translation, { status: 'summary' }));
  assert.throws(() => assertReviewedTranslation('reference/example.md', source, translation, { status: 'summary' }, true), /not fully reviewed/);
});

test('translation checks reject missing API literals, changed defaults and executable examples', () => {
  const source = '# Example\n\nUse `value`.\n\n| Prop | Type | Default | Description |\n| --- | --- | --- | --- |\n| `value` | `number` | 0 | Progress |\n\n```json\n{"version":1}\n```';
  const translation = source.replace('# Example', '# 示例').replace('Use', '使用').replace('Progress', '进度');
  assert.doesNotThrow(() => assertTranslationContractParity('example.md', source, translation));
  assert.throws(() => assertTranslationContractParity('example.md', source, translation.replace(/`value`/g, '`amount`')), /Contract literals differ/);
  assert.throws(() => assertTranslationContractParity('example.md', source, translation.replace('| 0 |', '| 100 |')), /types or defaults differ/);
  assert.throws(() => assertTranslationContractParity('example.md', source, translation.replace('"version":1', '"version":2')), /code examples differ/);
});

test('a preserved slug survives source relocation and reference indexes keep their URLs', () => {
  assert.equal(pageRoute('docs/new/location.md', '---\nslug: /old-location\n---\n# Title'), '/old-location');
  assert.equal(pageRoute('reference/ui-kit/components/README.md', '# Components'), '/reference/ui-kit/components');
  assert.equal(pageRoute('docs/index.md', '# Home'), '/');
  assert.throws(() => pageRoute('docs/page.md', '---\nslug: ../ambiguous\n---'), /absolute slug/);
});
