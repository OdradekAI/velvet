import assert from 'node:assert/strict';

const { createUiKitStoryHostAdapter } = await import('/tmp/velvet-plugin-tests/app/ui/story-registry.js');

const successHost = createUiKitStoryHostAdapter();
const success = successHost.createAsyncOperation('preview.success');
success.start();
success.resolve('ready');
assert.equal(await success.promise, 'ready');
assert.deepEqual(success.history, ['requested', 'pending', 'committed']);

const cancellationHost = createUiKitStoryHostAdapter();
const cancelled = cancellationHost.createAsyncOperation('preview.cancel');
cancelled.start();
const cancelledPromise = cancelled.promise;
cancelled.cancel();
await assert.rejects(cancelledPromise, /cancelled/);
assert.equal(cancelled.state, 'cancelled');

const timeoutHost = createUiKitStoryHostAdapter();
const timedOut = timeoutHost.createAsyncOperation('preview.timeout');
timedOut.start();
const timedOutPromise = timedOut.promise;
timedOut.timeout();
await assert.rejects(timedOutPromise, /timed out/);
assert.equal(timedOut.state, 'timed-out');

const failureHost = createUiKitStoryHostAdapter();
const failed = failureHost.createAsyncOperation('preview.failure');
failed.start();
const failedPromise = failed.promise;
failed.fail(new Error('provider failed'));
await assert.rejects(failedPromise, /provider failed/);
assert.equal(failed.state, 'failed');

const retryHost = createUiKitStoryHostAdapter();
const retried = retryHost.createAsyncOperation('preview.retry');
retried.start();
const firstAttempt = retried.promise;
retried.fail(new Error('first attempt failed'));
await assert.rejects(firstAttempt, /first attempt failed/);
retried.retry();
assert.equal(retried.state, 'pending');
retried.resolve('recovered');
assert.equal(await retried.promise, 'recovered');
assert.deepEqual(retried.history, ['requested', 'pending', 'failed', 'retrying', 'pending', 'committed']);

assert.deepEqual(
  retryHost.calls.map(({ method }) => method),
  ['preview.retry.requested', 'preview.retry.pending', 'preview.retry.failed', 'preview.retry.retrying', 'preview.retry.pending', 'preview.retry.committed'],
);

console.log('UI Kit async lifecycle tests passed (success, cancelled, timeout, failure, retry).');
