import type { UiKitStory, UiKitStoryAsyncOperation } from '../../story-registry.js';
import { VelvetText } from '../../primitives/index.js';
import { VelvetPreviewHost } from '../../host-projections/index.js';

const noop = () => undefined;
let activeOperation: UiKitStoryAsyncOperation<string> | undefined;
let retried = 0;
let openedExternally = false;

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
  { id: 'preview-host/error', componentId: 'preview-host', name: 'Error', description: 'Retryable preview error.', render: () => <VelvetPreviewHost state="error" message="Provider failed" onRetry={noop} />, scenario: { covers: ['retry'], actions: [], checkpoints: [{ name: 'error announcement', assert: (runtime) => { if (runtime.root.querySelector('[role="region"]')?.getAttribute('data-preview-state') !== 'error' || runtime.root.querySelector('[role="alert"]') === null || runtime.root.querySelector('button') === null) throw new Error('PreviewHost error accessibility contract failed'); } }] } },
  {
    id: 'preview-host/toolbar-mode',
    componentId: 'preview-host',
    name: 'Toolbar and mode',
    description: 'Provider toolbar and external-open action remain host-owned projections.',
    render: () => <VelvetPreviewHost mode="actual" onOpenExternal={() => { openedExternally = true; }} state="ready" toolbar={<button data-preview-toolbar-action="rotate" type="button">Rotate</button>}><VelvetText>Preview</VelvetText></VelvetPreviewHost>,
    scenario: {
      covers: ['open-external'],
      setup: () => { openedExternally = false; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[data-preview-action="open-external"]')?.click(); })],
      checkpoints: [{ name: 'mode and toolbar', afterAction: 0, assert: (runtime) => {
        if (runtime.root.querySelector('[data-preview-mode="actual"]') === null || runtime.root.querySelector('[role="toolbar"] [data-preview-toolbar-action="rotate"]') === null || !openedExternally) throw new Error('PreviewHost toolbar/mode contract failed');
      } }],
    },
  },
  {
    id: 'preview-host/retry',
    componentId: 'preview-host',
    name: 'Retry',
    description: 'Error fallback retry action.',
    render: () => <VelvetPreviewHost state="error" message="Provider failed" onRetry={() => { retried += 1; }} />,
    scenario: {
      covers: ['retry'],
      setup: () => { retried = 0; },
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('button')?.click(); })],
      checkpoints: [{ name: 'retry action', afterAction: 0, assert: () => { if (retried !== 1) throw new Error('PreviewHost retry contract failed'); } }],
    },
  },
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
      covers: ['retry'],
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
