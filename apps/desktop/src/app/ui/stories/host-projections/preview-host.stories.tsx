import type { UiKitStory, UiKitStoryAsyncOperation } from '../../story-registry.js';
import { VelvetText } from '../../primitives/index.js';
import { VelvetPreviewHost } from '../../host-projections/index.js';

const noop = () => undefined;
let activeOperation: UiKitStoryAsyncOperation<string> | undefined;

function operation(): UiKitStoryAsyncOperation<string> {
  if (!activeOperation) {
    throw new Error('Preview async story operation has not been set up');
  }
  return activeOperation;
}

function setupOperation(runtime: { host: { createAsyncOperation<T>(method: string): UiKitStoryAsyncOperation<T> } }): void {
  activeOperation = runtime.host.createAsyncOperation<string>('preview.load');
  activeOperation.start();
}

function clearOperation(): void {
  activeOperation = undefined;
}

export const previewHostStories: readonly UiKitStory[] = [
  { id: 'preview-host/ready', componentId: 'preview-host', name: 'Ready', description: 'Ready preview.', render: () => <VelvetPreviewHost state="ready"><VelvetText>Preview</VelvetText></VelvetPreviewHost> },
  { id: 'preview-host/error', componentId: 'preview-host', name: 'Error', description: 'Retryable preview error.', render: () => <VelvetPreviewHost state="error" message="Provider failed" onRetry={noop} /> },
  {
    id: 'preview-host/async-success',
    componentId: 'preview-host',
    name: 'Async success',
    description: 'Preview request commits successfully.',
    render: () => <VelvetPreviewHost state="ready"><VelvetText>Loaded preview</VelvetText></VelvetPreviewHost>,
    scenario: {
      asyncLifecycle: 'success',
      setup: setupOperation,
      actions: [(runtime) => runtime.act(async () => { operation().resolve('preview'); await operation().promise; })],
      checkpoints: [{ name: 'request committed', afterAction: 0, assert: () => { if (operation().state !== 'committed') throw new Error('Preview success lifecycle contract failed'); } }],
      cleanup: clearOperation,
    },
  },
  {
    id: 'preview-host/async-cancelled',
    componentId: 'preview-host',
    name: 'Async cancelled',
    description: 'Preview request is cancelled by its owner.',
    render: () => <VelvetPreviewHost state="empty" message="Preview cancelled" />,
    scenario: {
      asyncLifecycle: 'cancelled',
      setup: setupOperation,
      actions: [(runtime) => runtime.act(async () => { operation().cancel(); await operation().promise.catch(() => undefined); })],
      checkpoints: [{ name: 'request cancelled', afterAction: 0, assert: () => { if (operation().state !== 'cancelled') throw new Error('Preview cancellation lifecycle contract failed'); } }],
      cleanup: clearOperation,
    },
  },
  {
    id: 'preview-host/async-timeout',
    componentId: 'preview-host',
    name: 'Async timeout',
    description: 'Preview request reaches its timeout boundary.',
    render: () => <VelvetPreviewHost state="error" message="Preview timed out" />,
    scenario: {
      asyncLifecycle: 'timeout',
      setup: setupOperation,
      actions: [(runtime) => runtime.act(async () => { operation().timeout(); await operation().promise.catch(() => undefined); })],
      checkpoints: [{ name: 'request timed out', afterAction: 0, assert: () => { if (operation().state !== 'timed-out') throw new Error('Preview timeout lifecycle contract failed'); } }],
      cleanup: clearOperation,
    },
  },
  {
    id: 'preview-host/async-failure',
    componentId: 'preview-host',
    name: 'Async failure',
    description: 'Preview provider rejects the request.',
    render: () => <VelvetPreviewHost state="error" message="Provider failed" />,
    scenario: {
      asyncLifecycle: 'failure',
      setup: setupOperation,
      actions: [(runtime) => runtime.act(async () => { operation().fail(new Error('provider failed')); await operation().promise.catch(() => undefined); })],
      checkpoints: [{ name: 'request failed', afterAction: 0, assert: () => { if (operation().state !== 'failed') throw new Error('Preview failure lifecycle contract failed'); } }],
      cleanup: clearOperation,
    },
  },
  {
    id: 'preview-host/async-retry',
    componentId: 'preview-host',
    name: 'Async retry',
    description: 'A failed preview request retries and then succeeds.',
    render: () => <VelvetPreviewHost state="loading" />,
    scenario: {
      asyncLifecycle: 'retry',
      setup: setupOperation,
      actions: [
        (runtime) => runtime.act(async () => { operation().fail(new Error('provider failed')); await operation().promise.catch(() => undefined); }),
        (runtime) => runtime.act(async () => { operation().retry(); }),
        (runtime) => runtime.act(async () => { operation().resolve('preview'); await operation().promise; }),
      ],
      checkpoints: [
        { name: 'initial request failed', afterAction: 0, assert: () => { if (operation().state !== 'failed') throw new Error('Preview retry failure boundary contract failed'); } },
        { name: 'retry pending', afterAction: 1, assert: () => { if (operation().state !== 'pending') throw new Error('Preview retry pending contract failed'); } },
        { name: 'retry committed', afterAction: 2, assert: () => { if (operation().state !== 'committed') throw new Error('Preview retry success contract failed'); } },
      ],
      cleanup: clearOperation,
    },
  },
];
