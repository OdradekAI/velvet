import type { CoreTransport, Zone } from '../protocol.js';
import {
  ContributionRegistry,
  namespaceFor,
  type ContributionDescriptor,
} from '../contributions/contribution-registry.js';
import {
  WorkbenchAppRegistry,
} from '../apps/workbench-app.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import { canUsePermission } from '../zones/zone-policy.js';
import { PluginUiRegistry } from './ui-bridge.js';

export type PluginRuntime = 'builtin' | 'wasm' | 'sidecar';

export interface AppTarget {
  readonly appId: WorkbenchAppId;
  readonly apiRange: string;
}

export type ManifestContribution = Omit<
  ContributionDescriptor,
  'extensionId' | 'namespace'
> & {
  readonly namespace?: string;
};

export interface ExtensionManifest {
  readonly id: string;
  readonly version: string;
  readonly hostApi: string;
  readonly runtime?: PluginRuntime;
  readonly targetApp?: AppTarget;
  readonly contributions: readonly ManifestContribution[];
  readonly permissions: Partial<Record<Zone, readonly string[]>>;
}

export type PluginStatus = 'registered' | 'active' | 'disabled' | 'failed';

export interface PluginSummary {
  readonly id: string;
  readonly version: string;
  readonly status: PluginStatus;
  readonly error?: string;
}

interface PluginRecord {
  readonly manifest: ExtensionManifest;
  status: PluginStatus;
  registered: ContributionDescriptor[];
  error?: string;
}

export class PluginLifecycleError extends Error {
  public constructor(
    message: string,
    public readonly extensionId: string,
    public readonly cause?: unknown,
  ) {
    super(message);
    this.name = 'PluginLifecycleError';
  }
}

export class PluginClient {
  private readonly records = new Map<string, PluginRecord>();
  public readonly ui: PluginUiRegistry;

  public constructor(
    private readonly _core: CoreTransport,
    private readonly contributionRegistry: ContributionRegistry = new ContributionRegistry(),
    private readonly appRegistry: WorkbenchAppRegistry = new WorkbenchAppRegistry(),
    private readonly hostApiVersion = '1',
    ui: PluginUiRegistry = new PluginUiRegistry(),
  ) {
    this.ui = ui;
  }

  public register(manifest: ExtensionManifest): void {
    if (this.records.has(manifest.id)) {
      throw new PluginLifecycleError(
        `Extension is already registered: ${manifest.id}`,
        manifest.id,
      );
    }

    this.validateManifest(manifest);
    this.records.set(manifest.id, {
      manifest,
      status: 'registered',
      registered: [],
    });
  }

  public activate(extensionId: string): void {
    const record = this.require(extensionId);
    if (record.status === 'active') {
      return;
    }

    const registered: ContributionDescriptor[] = [];
    try {
      for (const contribution of record.manifest.contributions) {
        const descriptor = this.toDescriptor(record.manifest, contribution);
        this.contributionRegistry.register(descriptor);
        registered.push(descriptor);
      }
      record.registered = registered;
      record.status = 'active';
      record.error = undefined;
    } catch (cause) {
      for (const contribution of registered) {
        this.contributionRegistry.unregister(contribution);
      }
      record.registered = [];
      record.status = 'failed';
      record.error = cause instanceof Error ? cause.message : String(cause);
      throw new PluginLifecycleError(
        `Extension activation failed: ${extensionId}`,
        extensionId,
        cause,
      );
    }
  }

  public async deactivate(
    extensionId: string,
    _reason: 'user' | 'crash' | 'incompatible' = 'user',
  ): Promise<void> {
    const record = this.require(extensionId);
    for (const contribution of record.registered) {
      this.contributionRegistry.unregister(contribution);
    }
    await this.ui.disposeExtension(extensionId, _reason === 'user' ? 'unmount' : _reason);
    record.registered = [];
    record.status = 'disabled';
  }

  public async unregister(extensionId: string): Promise<boolean> {
    const record = this.records.get(extensionId);
    if (!record) {
      return false;
    }

    if (record.status === 'active') {
      await this.deactivate(extensionId);
    }
    return this.records.delete(extensionId);
  }

  public get(extensionId: string): PluginSummary | undefined {
    const record = this.records.get(extensionId);
    if (!record) {
      return undefined;
    }

    return this.summary(record);
  }

  public discover(): readonly PluginSummary[] {
    return [...this.records.values()].map((record) => this.summary(record));
  }

  public listVisible(activeAppId?: WorkbenchAppId): readonly ContributionDescriptor[] {
    const activeExtensions = new Set(
      [...this.records.values()]
        .filter((record) => record.status === 'active')
        .map((record) => record.manifest.id),
    );

    const visible = activeAppId === undefined
      ? this.contributionRegistry.list().filter(
          (contribution) => contribution.scope !== 'workbench-app',
        )
      : this.contributionRegistry.list({ activeAppId });

    return visible
      .filter((contribution) => activeExtensions.has(contribution.extensionId));
  }

  /** Compatibility check retained for the original zone-only skeleton. */
  public validateZone(manifest: ExtensionManifest): void {
    for (const contribution of manifest.contributions) {
      if (contribution.zone && !['navigation', 'contextual', 'operations'].includes(contribution.zone)) {
        throw new Error(`Unsupported extension zone: ${contribution.zone}`);
      }

      for (const permission of manifest.permissions[contribution.zone ?? 'navigation'] ?? []) {
        if (contribution.zone && !canUsePermission(contribution.zone, permission)) {
          throw new Error(
            `Permission ${permission} is not allowed in zone ${contribution.zone}`,
          );
        }
      }
    }
  }

  private validateManifest(manifest: ExtensionManifest): void {
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(manifest.id)) {
      throw new PluginLifecycleError(`Invalid extension id: ${manifest.id}`, manifest.id);
    }

    if (!apiRangeMatches(manifest.hostApi, this.hostApiVersion)) {
      throw new PluginLifecycleError(
        `Extension requires host API ${manifest.hostApi}; host provides ${this.hostApiVersion}`,
        manifest.id,
      );
    }

    this.validateZone(manifest);
    const appIds = new Set<WorkbenchAppId>();
    for (const contribution of manifest.contributions) {
      if (contribution.scope === 'workbench-app') {
        if (!contribution.appId) {
          throw new PluginLifecycleError(
            `App contribution ${contribution.id} must declare an app id`,
            manifest.id,
          );
        }
        appIds.add(contribution.appId);
      }
    }

    if (appIds.size > 1) {
      throw new PluginLifecycleError(
        'One extension cannot target multiple WorkbenchApp namespaces',
        manifest.id,
      );
    }

    if (manifest.targetApp) {
      const app = this.appRegistry.get(manifest.targetApp.appId);
      if (!app) {
        throw new PluginLifecycleError(
          `Target WorkbenchApp is not installed: ${manifest.targetApp.appId}`,
          manifest.id,
        );
      }
      if (!apiRangeMatches(manifest.targetApp.apiRange, app.apiVersion ?? '1')) {
        throw new PluginLifecycleError(
          `Target WorkbenchApp ${manifest.targetApp.appId} is incompatible with ${manifest.targetApp.apiRange}`,
          manifest.id,
        );
      }
      if (appIds.size === 1 && !appIds.has(manifest.targetApp.appId)) {
        throw new PluginLifecycleError(
          `Contribution namespace does not match target WorkbenchApp ${manifest.targetApp.appId}`,
          manifest.id,
        );
      }
    } else if (appIds.size === 1) {
      throw new PluginLifecycleError(
        'WorkbenchApp contributions require an explicit targetApp declaration',
        manifest.id,
      );
    }
  }

  private toDescriptor(
    manifest: ExtensionManifest,
    contribution: ManifestContribution,
  ): ContributionDescriptor {
    const { namespace: declaredNamespace, ...descriptor } = contribution;
    const namespace = namespaceFor(
      descriptor.scope,
      descriptor.appId,
      manifest.id,
    );
    if (declaredNamespace && declaredNamespace !== namespace) {
      throw new PluginLifecycleError(
        `Contribution ${descriptor.id} declares an invalid namespace`,
        manifest.id,
      );
    }

    return {
      ...descriptor,
      extensionId: manifest.id,
      namespace,
    };
  }

  private require(extensionId: string): PluginRecord {
    const record = this.records.get(extensionId);
    if (!record) {
      throw new PluginLifecycleError(
        `Extension is not registered: ${extensionId}`,
        extensionId,
      );
    }

    return record;
  }

  private summary(record: PluginRecord): PluginSummary {
    return {
      id: record.manifest.id,
      version: record.manifest.version,
      status: record.status,
      ...(record.error ? { error: record.error } : {}),
    };
  }
}

function apiRangeMatches(range: string, version: string): boolean {
  const normalizedRange = range.trim();
  if (normalizedRange === '*' || normalizedRange === version) {
    return true;
  }

  if (normalizedRange.startsWith('^')) {
    return normalizedRange.slice(1).split('.')[0] === version.split('.')[0];
  }

  return false;
}
