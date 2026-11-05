export const EXTENSION_ID = 'com.acme.asset-inspector';
export const JOB_KIND = `${EXTENSION_ID}.job.asset-quality-analysis`;
export const FACET_NAMESPACE = `${EXTENSION_ID}/quality`;
export const FACET_SCHEMA_VERSION = 1;

const CONTRIBUTIONS = {
  command: 'com.acme.asset-inspector.command.analyze-selection',
  menu: 'com.acme.asset-inspector.menu.asset.analyze',
  keybinding: 'com.acme.asset-inspector.keybinding.analyze-selection',
  resultsView: 'com.acme.asset-inspector.view.results',
  inspectorView: 'com.acme.asset-inspector.view.inspector',
  jobsView: 'com.acme.asset-inspector.view.jobs',
  preview: 'com.acme.asset-inspector.preview.quality',
};

export async function activate(host) {
  const disposables = [
    host.commands.register(CONTRIBUTIONS.command, (context) => analyzeSelection(host, context)),
    host.menus.register(CONTRIBUTIONS.menu),
    host.keybindings.register(CONTRIBUTIONS.keybinding),
    host.views.register(CONTRIBUTIONS.resultsView),
    host.views.register(CONTRIBUTIONS.inspectorView),
    host.views.register(CONTRIBUTIONS.jobsView),
    host.preview.registerAugmentation(CONTRIBUTIONS.preview, (context) => previewSummary(host, context)),
  ];

  return {
    async analyzeSelection(context) {
      return analyzeSelection(host, context);
    },
    async migrateStorage(fromSchemaVersion) {
      return host.storage.migrate(fromSchemaVersion, FACET_SCHEMA_VERSION);
    },
    dispose() {
      for (const dispose of disposables.reverse()) {
        dispose();
      }
    },
  };
}

async function analyzeSelection(host, context) {
  const selection = context?.selection ?? [];
  if (selection.length === 0) {
    throw new Error('Asset Inspector requires at least one selected asset');
  }

  const assets = await Promise.all(selection.map((asset) => host.assets.readProjection(asset.id)));
  const input = {
    assets: assets.map(({ id, contentKind, fingerprint }) => ({ id, contentKind, fingerprint })),
    selectionRevision: context.revision,
    configVersion: await host.configuration.version(),
  };
  const job = await host.jobs.create({
    kind: JOB_KIND,
    input,
    provenance: {
      ownerId: EXTENSION_ID,
      providerId: EXTENSION_ID,
      providerVersion: '0.1.0',
      inputFingerprint: assets.map((asset) => `${asset.id}:${asset.fingerprint}`).join('|'),
      schemaVersion: FACET_SCHEMA_VERSION,
    },
  });

  try {
    const completed = await host.jobs.wait(job.id);
    if (completed.status === 'cancelled') {
      host.diagnostics.log({ code: 'asset-inspector.job-cancelled', jobId: job.id, retryable: true });
      return completed;
    }
    if (completed.status !== 'completed') {
      throw new Error(`Asset analysis ended in ${completed.status}`);
    }

    for (const result of completed.result.findings) {
      await host.facets.write({
        namespace: FACET_NAMESPACE,
        schemaVersion: FACET_SCHEMA_VERSION,
        assetId: result.assetId,
        expectedFingerprint: result.inputFingerprint,
        value: result,
      });
    }
    host.notifications.show({ severity: 'success', message: `Analyzed ${assets.length} asset(s)` });
    return completed;
  } catch (error) {
    host.diagnostics.log({
      code: error?.code === 'stale-fingerprint'
        ? 'asset-inspector.stale-fingerprint'
        : 'asset-inspector.analysis-failed',
      jobId: job.id,
      retryable: error?.code !== 'stale-fingerprint',
    });
    host.notifications.show({ severity: 'error', message: 'Asset analysis failed; retry from Operations' });
    throw error;
  }
}

async function previewSummary(host, context) {
  const facet = await host.facets.read({
    namespace: FACET_NAMESPACE,
    assetId: context.assetId,
  });
  return facet ? { slot: 'metadata', summary: facet.value.summary } : undefined;
}
