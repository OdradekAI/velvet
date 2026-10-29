import assert from 'node:assert/strict';
import {
  FocusService,
  focusTargetId,
  parseFocusTargetId,
} from '../src/workbench/focus/focus-service.ts';

const service = new FocusService();
const workspace = { kind: 'part', part: 'workspace-area' };
const view = { kind: 'view', viewId: 'sample.extension.reader' };

assert.equal(focusTargetId(view), 'view:sample.extension.reader');
assert.deepEqual(parseFocusTargetId('view:sample.extension.reader'), view);
assert.deepEqual(parseFocusTargetId('part:workspace-area'), workspace);
assert.equal(parseFocusTargetId('part:not-a-part'), undefined);

service.focus(workspace);
service.focus(view);
assert.deepEqual(service.snapshot().previous, workspace);
service.restore();
assert.deepEqual(service.snapshot().current, workspace);
assert.equal(service.snapshot().previous, undefined);

service.focus(view);
service.clear();
assert.deepEqual(service.snapshot().previous, view);
assert.equal(service.snapshot().current, undefined);

console.log('Focus service tests passed.');
