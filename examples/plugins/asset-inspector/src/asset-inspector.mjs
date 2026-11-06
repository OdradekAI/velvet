export const EXTENSION_ID = 'com.acme.asset-inspector';
export const VERSION = '0.1.0';
export const HOST_API_RANGE = '^1';
export const UI_KIT_API_VERSION = '1.0';
export const UI_COMPONENT_CONTRACT_VERSION = 1;
export const JOB_KIND = `${EXTENSION_ID}.job.asset-quality-analysis`;
export const FACET_NAMESPACE = `${EXTENSION_ID}/quality`;
export const FACET_SCHEMA_VERSION = 1;

export const UI_COMPONENTS = Object.freeze([
  'text',
  'button',
  'table',
  'progress',
  'empty-state',
  'error-state',
  'loading-state',
]);

const CONTRIBUTIONS = {
  command: 'com.acme.asset-inspector.command.analyze-selection',
  menu: 'com.acme.asset-inspector.menu.asset.analyze',
  keybinding: 'com.acme.asset-inspector.keybinding.analyze-selection',
  resultsView: 'com.acme.asset-inspector.view.results',
  inspectorView: 'com.acme.asset-inspector.view.inspector',
  jobsView: 'com.acme.asset-inspector.view.jobs',
  preview: 'com.acme.asset-inspector.preview.quality',
};

const STORAGE_KEY = 'settings';
const DEFAULT_SETTINGS = Object.freeze({ qualityThreshold: 0.8 });

export class AssetInspectorError extends Error {
  constructor(code, message, details = undefined) {
    super(message);
    this.name = 'AssetInspectorError';
    this.code = code;
    this.details = details;
  }
}

/**
 * Validates the stable fields consumed by this example. The host performs the
 * authoritative schema, permission and target-app validation; this check is a
 * useful early error for a developer running the package locally.
 */
export function validateManifest(manifest) {
  if (!manifest || typeof manifest !== 'object') {
    throw new AssetInspectorError('manifest-invalid', 'Manifest must be an object');
  }
  if (manifest.id !== EXTENSION_ID) {
    throw new AssetInspectorError('manifest-invalid', `Expected extension id ${EXTENSION_ID}`);
  }
  if (manifest.version !== VERSION) {
    throw new AssetInspectorError('manifest-invalid', `Expected demo version ${VERSION}`);
  }
  if (manifest.hostApi !== HOST_API_RANGE || manifest.bridgeApi !== '^1') {
    throw new AssetInspectorError(
      'manifest-incompatible',
      `Asset Inspector requires host ${HOST_API_RANGE} and bridge ^1`,
    );
  }
  const contributions = Array.isArray(manifest.contributions) ? manifest.contributions : [];
  const ids = new Set(contributions.map((contribution) => contribution.id));
  for (const id of Object.values(CONTRIBUTIONS)) {
    if (!ids.has(id)) {
      throw new AssetInspectorError('manifest-invalid', `Missing contribution ${id}`);
    }
  }
  if (!manifest.storage || manifest.storage.schemaVersion !== FACET_SCHEMA_VERSION) {
    throw new AssetInspectorError('manifest-invalid', 'Storage schema version must be 1');
  }
  return manifest;
}

function majorVersion(value) {
  const match = String(value ?? '').match(/^(?:\^|~)?(\d+)/);
  return match ? Number(match[1]) : undefined;
}

function requireCompatibleHost(host) {
  const hostApiVersion = host?.apiVersion ?? '1';
  if (majorVersion(hostApiVersion) !== majorVersion(HOST_API_RANGE)) {
    throw new AssetInspectorError(
      'host-api-incompatible',
      `Asset Inspector requires host API ${HOST_API_RANGE}; received ${hostApiVersion}`,
      { required: HOST_API_RANGE, received: hostApiVersion },
    );
  }
  if (host?.ui?.apiVersion !== UI_KIT_API_VERSION) {
    throw new AssetInspectorError(
      'ui-kit-incompatible',
      `Asset Inspector requires UI Kit ${UI_KIT_API_VERSION}`,
      { required: UI_KIT_API_VERSION, received: host?.ui?.apiVersion ?? null },
    );
  }
  if (host?.ui?.componentContractVersion !== UI_COMPONENT_CONTRACT_VERSION) {
    throw new AssetInspectorError(
      'ui-kit-incompatible',
      `Asset Inspector requires UI component contract ${UI_COMPONENT_CONTRACT_VERSION}`,
      {
        required: UI_COMPONENT_CONTRACT_VERSION,
        received: host?.ui?.componentContractVersion ?? null,
      },
    );
  }
}

function capability(host, name) {
  if (!host.capabilities?.require) {
    throw new AssetInspectorError('capability-unavailable', `Capability broker is unavailable: ${name}`);
  }
  return host.capabilities.require(name);
}

function safeLog(host, record) {
  try {
    capability(host, 'logs.write');
    host.diagnostics?.log(record);
  } catch {
    // Logging is diagnostic-only and must never replace the domain error.
  }
}

function safeNotify(host, record) {
  try {
    capability(host, 'notifications.show');
    host.notifications?.show(record);
  } catch {
    // A closed or unavailable notification surface must not fail a Job.
  }
}

function safeRender(host, component, props) {
  try {
    host.ui?.render({
      surfaceId: CONTRIBUTIONS.inspectorView,
      component,
      props,
    });
  } catch (error) {
    safeLog(host, {
      code: 'asset-inspector.ui-render-failed',
      level: 'warn',
      message: error instanceof Error ? error.message : String(error),
      component,
    });
  }
}

function errorCode(error) {
  return typeof error?.code === 'string' ? error.code : 'asset-inspector.analysis-failed';
}

async function readSettings(host) {
  const stored = await host.storage.read(STORAGE_KEY);
  if (!stored || typeof stored !== 'object') {
    return { ...DEFAULT_SETTINGS };
  }
  const threshold = Number(stored.qualityThreshold);
  return Number.isFinite(threshold) && threshold >= 0 && threshold <= 1
    ? { qualityThreshold: threshold }
    : { ...DEFAULT_SETTINGS };
}

async function previewSummary(host, context) {
  capability(host, 'preview.render');
  const facet = await host.facets.read({
    namespace: FACET_NAMESPACE,
    assetId: context.assetId,
  });
  return facet ? { slot: 'metadata', summary: facet.value.summary } : undefined;
}

async function analyzeSelection(host, context, options = {}) {
  capability(host, 'asset.read');
  capability(host, 'jobs.create');
  const hostContext = host.context ? await host.context.read() : undefined;
  const selection = context?.selection ?? hostContext?.selection ?? [];
  if (selection.length === 0) {
    const error = new AssetInspectorError(
      'selection-empty',
      'Asset Inspector requires at least one selected asset',
    );
    safeRender(host, 'empty-state', { message: error.message });
    throw error;
  }

  safeRender(host, 'loading-state', { message: 'Reading selected assets…' });
  const assets = await Promise.all(selection.map((asset) => host.assets.readProjection(asset.id)));
  capability(host, 'storage.read');
  capability(host, 'config.read');
  const settings = await readSettings(host);
  const input = {
    assets: assets.map(({ id, contentKind, fingerprint }) => ({ id, contentKind, fingerprint })),
    selectionRevision: context?.revision ?? hostContext?.revision ?? 0,
    configVersion: await host.configuration.version(),
    qualityThreshold: settings.qualityThreshold,
  };
  const job = await host.jobs.create({
    kind: JOB_KIND,
    input,
    provenance: {
      ownerId: EXTENSION_ID,
      providerId: EXTENSION_ID,
      providerVersion: VERSION,
      inputFingerprint: assets.map((asset) => `${asset.id}:${asset.fingerprint}`).join('|'),
      schemaVersion: FACET_SCHEMA_VERSION,
    },
  });

  if (options.signal?.aborted) {
    await host.jobs.cancel(job.id);
  }

  try {
    capability(host, 'jobs.read');
    const completed = await host.jobs.wait(job.id, {
      onProgress: (snapshot) => {
        safeRender(host, 'progress', {
          value: snapshot.value ?? snapshot.progress ?? 0,
          message: snapshot.message ?? 'Analyzing selected assets…',
          state: snapshot.state ?? 'running',
        });
      },
    });
    if (completed.status === 'cancelled') {
      safeLog(host, {
        code: 'asset-inspector.job-cancelled',
        level: 'info',
        jobId: job.id,
        retryable: true,
      });
      safeRender(host, 'empty-state', { message: 'Analysis cancelled' });
      return completed;
    }
    if (completed.status !== 'completed') {
      throw new AssetInspectorError(
        'job-failed',
        `Asset analysis ended in ${completed.status}`,
        { jobId: job.id, status: completed.status },
      );
    }
    if (!completed.result || !Array.isArray(completed.result.findings)) {
      throw new AssetInspectorError('job-result-invalid', 'Asset analysis returned no findings');
    }

    capability(host, 'asset.facet.write');
    for (const result of completed.result.findings) {
      await host.facets.write({
        namespace: FACET_NAMESPACE,
        schemaVersion: FACET_SCHEMA_VERSION,
        assetId: result.assetId,
        expectedFingerprint: result.inputFingerprint,
        value: result,
      });
    }
    safeRender(host, 'table', {
      columns: ['assetId', 'summary', 'score'],
      rows: completed.result.findings,
    });
    safeNotify(host, {
      severity: 'success',
      message: `Analyzed ${assets.length} asset(s)`,
    });
    return completed;
  } catch (error) {
    const code = errorCode(error);
    safeLog(host, {
      code: code === 'stale-fingerprint'
        ? 'asset-inspector.stale-fingerprint'
        : code === 'permission-denied'
          ? 'asset-inspector.permission-denied'
          : 'asset-inspector.analysis-failed',
      level: 'error',
      jobId: job.id,
      retryable: code !== 'stale-fingerprint' && code !== 'permission-denied',
      message: error instanceof Error ? error.message : String(error),
    });
    safeRender(host, 'error-state', {
      message: code === 'permission-denied'
        ? 'Asset Inspector is not allowed to write quality results'
        : 'Asset analysis failed; retry from Operations',
      code,
    });
    safeNotify(host, {
      severity: 'error',
      message: code === 'permission-denied'
        ? 'Asset Inspector permission denied'
        : 'Asset analysis failed; retry from Operations',
    });
    throw error;
  }
}

export async function activate(host) {
  requireCompatibleHost(host);
  capability(host, 'command.register');
  capability(host, 'menu.register');
  capability(host, 'view.register');
  const disposables = [
    host.commands.register(CONTRIBUTIONS.command, (context) => analyzeSelection(host, context)),
    host.menus.register(CONTRIBUTIONS.menu),
    host.keybindings.register(CONTRIBUTIONS.keybinding),
    host.views.register(CONTRIBUTIONS.resultsView),
    host.views.register(CONTRIBUTIONS.inspectorView),
    host.views.register(CONTRIBUTIONS.jobsView),
    host.preview.registerAugmentation(CONTRIBUTIONS.preview, (context) => previewSummary(host, context)),
    host.ui.register({
      surfaceId: CONTRIBUTIONS.inspectorView,
      apiVersion: UI_KIT_API_VERSION,
      componentContractVersion: UI_COMPONENT_CONTRACT_VERSION,
      components: UI_COMPONENTS,
    }),
  ];

  return {
    async analyzeSelection(context, options) {
      return analyzeSelection(host, context, options);
    },
    async cancelAnalysis(jobId) {
      capability(host, 'jobs.create');
      return host.jobs.cancel(jobId);
    },
    async readSettings() {
      capability(host, 'storage.read');
      return readSettings(host);
    },
    async saveSettings(settings) {
      capability(host, 'storage.write');
      const qualityThreshold = Number(settings?.qualityThreshold);
      if (!Number.isFinite(qualityThreshold) || qualityThreshold < 0 || qualityThreshold > 1) {
        throw new AssetInspectorError('settings-invalid', 'qualityThreshold must be between 0 and 1');
      }
      return host.storage.write(STORAGE_KEY, { qualityThreshold });
    },
    async migrateStorage(fromSchemaVersion) {
      capability(host, 'storage.write');
      return host.storage.migrate(fromSchemaVersion, FACET_SCHEMA_VERSION);
    },
    dispose() {
      for (const dispose of disposables.reverse()) {
        dispose();
      }
    },
  };
}

export const contributionIds = Object.freeze({ ...CONTRIBUTIONS });
