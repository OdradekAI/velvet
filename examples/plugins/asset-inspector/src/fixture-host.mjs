import {
  AssetInspectorError,
  EXTENSION_ID,
  FACET_NAMESPACE,
  FACET_SCHEMA_VERSION,
  JOB_KIND,
  UI_COMPONENT_CONTRACT_VERSION,
  UI_KIT_API_VERSION,
} from './asset-inspector.mjs';

export function createAssetInspectorFixtureHost(options = {}) {
  const assets = new Map([
    ['asset-1', { id: 'asset-1', contentKind: 'image', fingerprint: 'sha256:asset-1:v1' }],
    ['asset-2', { id: 'asset-2', contentKind: 'text/plain', fingerprint: 'sha256:asset-2:v1' }],
  ]);
  const facets = new Map();
  const storage = new Map([['settings', { qualityThreshold: 0.8 }]]);
  const registrations = [];
  const uiRegistrations = new Map();
  const uiEvents = [];
  const logs = [];
  const notifications = [];
  const jobRequests = new Map();
  const pendingJobs = new Map();
  const deniedCapabilities = new Set(options.deniedCapabilities ?? []);
  const state = {
    assets,
    facets,
    storage,
    registrations,
    uiRegistrations,
    uiEvents,
    logs,
    notifications,
    jobRequests,
    pendingJobs,
    migrated: undefined,
    nextJobId: 1,
    nextJobMode: options.nextJobMode ?? 'completed',
    lastJobId: undefined,
    contextReads: 0,
    coreAssetReads: 0,
    capabilityChecks: [],
    crashCount: 0,
  };

  const register = (kind, id, handler = undefined) => {
    const entry = { kind, id, handler };
    registrations.push(entry);
    return () => {
      const index = registrations.indexOf(entry);
      if (index >= 0) {
        registrations.splice(index, 1);
      }
    };
  };

  const host = {
    apiVersion: options.apiVersion ?? '1',
    context: {
      read: async () => {
        state.contextReads += 1;
        return {
          revision: 4,
          selection: [{ id: 'asset-1', kind: 'asset' }],
          activeAppId: 'asset-manager',
        };
      },
    },
    capabilities: {
      require: (name) => {
        state.capabilityChecks.push(name);
        if (deniedCapabilities.has(name)) {
          const error = new AssetInspectorError(
            'permission-denied',
            `Capability denied: ${name}`,
            { capability: name, extensionId: EXTENSION_ID },
          );
          throw error;
        }
        return { capability: name, extensionId: EXTENSION_ID };
      },
    },
    commands: {
      register: (id, handler) => register('command', id, handler),
    },
    menus: {
      register: (id) => register('menu', id),
    },
    keybindings: {
      register: (id) => register('keybinding', id),
    },
    views: {
      register: (id) => register('view', id),
    },
    preview: {
      registerAugmentation: (id, handler) => register('preview', id, handler),
    },
    ui: {
      apiVersion: options.uiApiVersion ?? UI_KIT_API_VERSION,
      componentContractVersion: options.uiComponentContractVersion ?? UI_COMPONENT_CONTRACT_VERSION,
      register: (descriptor) => {
        if (descriptor.apiVersion !== UI_KIT_API_VERSION) {
          throw new AssetInspectorError('ui-kit-incompatible', 'Unsupported UI Kit version');
        }
        for (const component of descriptor.components) {
          if (!['text', 'button', 'table', 'progress', 'empty-state', 'error-state', 'loading-state'].includes(component)) {
            throw new AssetInspectorError('ui-component-denied', `Unsupported plugin UI component: ${component}`);
          }
        }
        uiRegistrations.set(descriptor.surfaceId, descriptor);
        return () => uiRegistrations.delete(descriptor.surfaceId);
      },
      render: (event) => {
        const descriptor = uiRegistrations.get(event.surfaceId);
        if (!descriptor || !descriptor.components.includes(event.component)) {
          throw new AssetInspectorError(
            'ui-component-denied',
            `Component ${event.component} is not declared for this surface`,
          );
        }
        uiEvents.push(event);
      },
    },
    assets: {
      readProjection: async (id) => {
        state.coreAssetReads += 1;
        const asset = assets.get(id);
        if (!asset) {
          throw new AssetInspectorError('asset-not-found', `Asset projection not found: ${id}`);
        }
        return { ...asset };
      },
    },
    configuration: {
      version: async () => 'demo-settings-v1',
    },
    jobs: {
      create: async (request) => {
        const id = `job-${state.nextJobId++}`;
        const mode = state.nextJobMode;
        state.nextJobMode = 'completed';
        const snapshot = { id, status: 'queued', request };
        jobRequests.set(id, { ...snapshot, mode });
        state.lastJobId = id;
        return snapshot;
      },
      wait: async (id, hooks = {}) => {
        const request = jobRequests.get(id);
        if (!request) {
          throw new AssetInspectorError('job-not-found', `Job not found: ${id}`);
        }
        if (request.mode === 'crash') {
          state.crashCount += 1;
          throw new AssetInspectorError('runtime-crash', 'Analysis runner exited unexpectedly');
        }
        if (request.mode === 'cancelled') {
          return { id, status: 'cancelled', result: null };
        }
        if (request.mode === 'unknown') {
          return { id, status: 'unknown', result: null };
        }
        if (request.mode === 'stale') {
          assets.get('asset-1').fingerprint = 'sha256:asset-1:v2';
        }
        if (request.mode === 'blocked') {
          return new Promise((resolve) => {
            pendingJobs.set(id, { resolve, hooks });
          });
        }
        hooks.onProgress?.({ id, value: 0.25, message: 'Reading asset projections', state: 'running' });
        hooks.onProgress?.({ id, value: 1, message: 'Quality analysis complete', state: 'completed' });
        return completedResult(id, request.request);
      },
      cancel: async (id) => {
        const pending = pendingJobs.get(id);
        if (pending) {
          pendingJobs.delete(id);
          pending.resolve({ id, status: 'cancelled', result: null });
        } else {
          const request = jobRequests.get(id);
          if (request) {
            request.mode = 'cancelled';
          }
        }
        return { id, status: 'cancelled' };
      },
    },
    facets: {
      write: async (record) => {
        const current = assets.get(record.assetId);
        if (!current || current.fingerprint !== record.expectedFingerprint) {
          const error = new AssetInspectorError('stale-fingerprint', 'asset fingerprint is stale');
          throw error;
        }
        facets.set(`${record.namespace}:${record.assetId}`, record);
      },
      read: async ({ namespace, assetId }) => facets.get(`${namespace}:${assetId}`),
    },
    storage: {
      read: async (key) => storage.get(key),
      write: async (key, value) => {
        storage.set(key, structuredClone(value));
        return value;
      },
      migrate: async (from, to) => {
        state.migrated = { from, to };
        return state.migrated;
      },
    },
    diagnostics: {
      log: (record) => {
        if (options.failLogs) {
          throw new Error('diagnostic sink unavailable');
        }
        logs.push(record);
      },
    },
    notifications: {
      show: (record) => {
        if (options.failNotifications) {
          throw new Error('notification surface unavailable');
        }
        notifications.push(record);
      },
    },
  };

  return { host, state };
}

function completedResult(id, request) {
  const first = request.input.assets[0];
  return {
    id,
    status: 'completed',
    result: {
      findings: [{
        assetId: first.id,
        inputFingerprint: first.fingerprint,
        summary: 'Readable asset',
        score: 0.98,
      }],
    },
  };
}
