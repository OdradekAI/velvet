import assert from 'node:assert/strict';
import { SourceRootService } from '/tmp/velvet-plugin-tests/workbench/services/source-root-service.js';

let roots = ['/Users/example/assets'];
const requests = [];
const service = new SourceRootService({
  async request(request) {
    requests.push(request);
    if (request.kind === 'query-approved-source-roots') {
      return { kind: 'approved-source-roots', roots };
    }
    if (request.kind === 'add-approved-source-root') {
      roots = [...roots, request.root];
      return { kind: 'accepted' };
    }
    if (request.kind === 'remove-approved-source-root') {
      roots = roots.filter((root) => root !== request.root);
      return { kind: 'accepted' };
    }
    throw new Error(`unexpected request ${request.kind}`);
  },
});

const updates = [];
service.subscribe(() => {
  throw new Error('one settings listener failed');
});
service.subscribe((next) => updates.push([...next]));
assert.deepEqual(await service.load(), ['/Users/example/assets']);
assert.deepEqual(await service.add('/Users/example/imports'), [
  '/Users/example/assets',
  '/Users/example/imports',
]);
assert.deepEqual(await service.remove('/Users/example/assets'), ['/Users/example/imports']);
assert.deepEqual(updates, [
  [],
  ['/Users/example/assets'],
  ['/Users/example/assets', '/Users/example/imports'],
  ['/Users/example/imports'],
]);
assert.equal(requests[0].kind, 'query-approved-source-roots');
await assert.rejects(service.add(''), /bounded non-empty path/);
await assert.rejects(service.remove(`bad\0path`), /bounded non-empty path/);

console.log('Source-root service tests passed.');
