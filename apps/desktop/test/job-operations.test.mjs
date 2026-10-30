import assert from 'node:assert/strict';
import { Window } from 'happy-dom';

const browser = new Window({ url: 'http://velvet.test/' });
for (const name of [
  'Document', 'Element', 'Event', 'HTMLElement', 'InputEvent', 'MouseEvent',
  'MutationObserver', 'Node', 'PointerEvent',
]) {
  if (browser[name]) globalThis[name] = browser[name];
}
globalThis.window = browser;
globalThis.document = browser.document;
Object.defineProperty(globalThis, 'navigator', { configurable: true, value: browser.navigator });
globalThis.IS_REACT_ACT_ENVIRONMENT = true;

const ReactModule = await import('react');
const React = ReactModule.default;
const { act } = ReactModule;
const { createRoot } = await import('react-dom/client');
const { JobOperationsPanel } = await import(
  '/tmp/velvet-plugin-tests/app/renderer/job-operations-panel.js',
);

let jobs = [];
let nextId = 1;
const listeners = new Set();
const notify = () => listeners.forEach((listener) => listener());
const client = {
  list: () => jobs,
  subscribe: (listener) => {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
  query: async () => jobs,
  create: async ({ kind, willFail }) => {
    const job = {
      id: `job-${String(nextId++).padStart(6, '0')}`,
      kind,
      status: 'running',
      progress: 10,
      message: 'Running',
      attempt: 1,
      retryable: true,
      durationMs: 2400,
      willFail,
      provenance: {
        ownerId: 'core',
        assetId: null,
        contentKind: null,
        inputFingerprint: null,
        providerId: null,
        providerVersion: null,
        settingsFingerprint: null,
        schemaVersion: 1,
      },
      result: null,
    };
    jobs = [...jobs, job];
    notify();
    return job;
  },
  cancel: async (id) => {
    jobs = jobs.map((job) => job.id === id ? { ...job, status: 'cancelled', message: 'Cancelled by user' } : job);
    notify();
    return jobs.find((job) => job.id === id);
  },
  retry: async (id) => {
    jobs = jobs.map((job) => job.id === id ? { ...job, status: 'running', progress: 0, attempt: job.attempt + 1 } : job);
    notify();
    return jobs.find((job) => job.id === id);
  },
};

const rootElement = document.createElement('div');
document.body.append(rootElement);
const root = createRoot(rootElement);
await act(async () => {
  root.render(React.createElement(JobOperationsPanel, { jobs: client }));
  await new Promise((resolve) => setTimeout(resolve, 10));
});
assert.match(rootElement.textContent ?? '', /No jobs yet/);

await act(async () => {
  rootElement.querySelector('[data-job-create="success"]').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelectorAll('[data-job-entry]').length, 1);
assert.equal(rootElement.querySelector('[data-job-status]').textContent, 'Running');

await act(async () => {
  rootElement.querySelector('[data-job-cancel]').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelector('[data-job-status]').textContent, 'Cancelled');
assert.ok(rootElement.querySelector('[data-job-retry]'));

await act(async () => {
  rootElement.querySelector('[data-job-retry]').click();
  await new Promise((resolve) => setTimeout(resolve, 0));
});
assert.equal(rootElement.querySelector('[data-job-status]').textContent, 'Running');

await act(async () => root.unmount());
