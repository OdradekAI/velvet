/**
 * Canonical desktop/core contract. This module deliberately has no React,
 * Vue, Tauri or feature-module dependency.
 */
export const PROTOCOL_VERSION = 1 as const;
export type ProtocolVersion = typeof PROTOCOL_VERSION;
export type WorkbenchAppId = string;
/** Compatibility alias for older clients and migration code. */
export type WorkspaceId = WorkbenchAppId;
export type Zone = 'navigation' | 'contextual' | 'operations';
export type Part =
  | 'title-bar'
  | 'activity-bar'
  | 'primary-sidebar'
  | 'workspace-area'
  | 'auxiliary-sidebar'
  | 'operations-panel'
  | 'status-bar'
  | 'overlay-layer';

export type LayoutCommandId =
  | 'workbench.activate-app'
  | 'workbench.set-part-visibility'
  | 'workbench.toggle-part'
  | 'workbench.focus-part'
  | 'workbench.resize-part'
  | 'workbench.collapse-part'
  | 'workbench.maximize-part'
  | 'workbench.upsert-view-container'
  | 'workbench.upsert-view'
  | 'workbench.set-view-container-visibility'
  | 'workbench.set-view-visibility'
  | 'workbench.move-view'
  | 'workbench.reorder-view-container'
  | 'workbench.remove-view-container'
  | 'workbench.reset-layout'
  | 'workbench.reset-app-layout'
  | 'workbench.reset-view-layout';

export type LayoutOrderAnchor =
  | { readonly position: 'before'; readonly id: string }
  | { readonly position: 'after'; readonly id: string };

export type LayoutCommand =
  | { readonly id: 'workbench.activate-app'; readonly appId: WorkbenchAppId }
  | {
      readonly id: 'workbench.set-part-visibility';
      readonly part: Part;
      readonly visible: boolean;
    }
  | { readonly id: 'workbench.toggle-part'; readonly part: Part }
  | { readonly id: 'workbench.focus-part'; readonly part: Part }
  | { readonly id: 'workbench.resize-part'; readonly part: Part; readonly size: number }
  | {
      readonly id: 'workbench.collapse-part';
      readonly part: Part;
      readonly collapsed: boolean;
    }
  | {
      readonly id: 'workbench.upsert-view-container';
      readonly containerId: string;
      readonly visible: boolean;
      readonly order: number;
    }
  | {
      readonly id: 'workbench.upsert-view';
      readonly viewId: string;
      readonly containerId: string;
      readonly visible: boolean;
      readonly order: number;
    }
  | {
      readonly id: 'workbench.set-view-container-visibility';
      readonly containerId: string;
      readonly visible: boolean;
    }
  | {
      readonly id: 'workbench.set-view-visibility';
      readonly viewId: string;
      readonly visible: boolean;
    }
  | { readonly id: 'workbench.maximize-part'; readonly part: Part }
  | {
      readonly id: 'workbench.move-view';
      readonly viewId: string;
      readonly targetContainerId: string;
      readonly anchor?: LayoutOrderAnchor;
    }
  | {
      readonly id: 'workbench.reorder-view-container';
      readonly containerId: string;
      readonly anchor?: LayoutOrderAnchor;
    }
  | { readonly id: 'workbench.remove-view-container'; readonly containerId: string }
  | { readonly id: 'workbench.reset-layout' }
  | { readonly id: 'workbench.reset-app-layout'; readonly appId: WorkbenchAppId }
  | { readonly id: 'workbench.reset-view-layout'; readonly viewId?: string };

export interface CorePluginCommandContext {
  readonly activeAppId: WorkbenchAppId | null;
  readonly keys: Readonly<Record<string, string | number | boolean | null>>;
}

export type CoreRequest =
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-layout' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-extensions' }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-plugin-projection';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'register-plugin';
      readonly manifest: CorePluginManifest;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'activate-plugin';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'disable-plugin';
      readonly extensionId: string;
      readonly reason: CorePluginDisableReason;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'check-plugin-health';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'recover-plugin';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'install-plugin';
      readonly sourceDir: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'inspect-plugin';
      readonly sourceDir: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'commit-plugin-install';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'rollback-plugin-install';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'uninstall-plugin';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'invoke-plugin-command';
      readonly extensionId: string;
      readonly commandId: string;
      readonly args: unknown;
      readonly context: CorePluginCommandContext;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'layout-command';
      readonly command: LayoutCommand;
      readonly expectedRevision?: number;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'resolve-preview';
      readonly assetId: string;
      readonly contentKind: string;
      readonly intent: 'quick' | 'inspect' | 'analyze';
      readonly surface: 'workspace' | 'thumbnail' | 'inspector';
    };

export type CoreResponse =
  | { readonly kind: 'layout'; readonly state: LayoutSnapshot }
  | { readonly kind: 'preview'; readonly preview: unknown }
  | { readonly kind: 'extensions'; readonly extensions: readonly CorePluginSummary[] }
  | { readonly kind: 'extension'; readonly extension: CorePluginSummary }
  | { readonly kind: 'plugin-install-preview'; readonly preview: CorePluginInstallPreview }
  | { readonly kind: 'plugin-projection'; readonly projection: CorePluginProjection }
  | { readonly kind: 'accepted' };

export type PluginState = 'registered' | 'starting' | 'active' | 'disabled' | 'failed';

export interface PluginSummary {
  readonly id: string;
  readonly version: string;
  readonly active: boolean;
  readonly state: PluginState;
}

export type CorePluginRuntime = 'built-in' | 'wasm' | 'sidecar';
export type CorePluginDisableReason = 'user' | 'crash' | 'incompatible';
export type CorePluginPermission =
  | 'workspace-open'
  | 'library-read'
  | 'source-connect'
  | 'asset-read'
  | 'metadata-read'
  | 'metadata-write'
  | 'preview-render'
  | 'jobs-create'
  | 'jobs-read'
  | 'logs-read'
  | 'external-process'
  | 'network';
export type CorePluginContributionKind =
  | 'activity-bar-entry'
  | 'companion-view'
  | 'operation-view'
  | 'view-container'
  | 'view'
  | 'dialog'
  | 'context-menu'
  | 'preview-provider'
  | 'preview-augmentation'
  | 'inspector'
  | 'command'
  | 'job';
export type CorePluginContributionScope =
  | { readonly kind: 'workbench' }
  | { readonly kind: 'workbench-app'; readonly appId: string }
  | { readonly kind: 'capability' };
export type CorePluginHostContainer =
  | 'activity-bar'
  | 'title-bar'
  | 'workspace-area'
  | 'primary-sidebar'
  | 'auxiliary-sidebar'
  | 'operations-panel'
  | 'status-bar'
  | 'overlay-layer'
  | 'context-menu'
  | 'command-palette';
export type CorePluginContextAnchor =
  | 'asset'
  | 'feed-item'
  | 'collection'
  | 'selection'
  | 'view'
  | 'background';
export type CorePluginUiSurfaceKind =
  | 'app-surface'
  | 'view'
  | 'dialog'
  | 'popover'
  | 'inspector';

export interface CorePluginContribution {
  readonly id: string;
  readonly kind: CorePluginContributionKind;
  readonly scope: CorePluginContributionScope;
  readonly namespace: string;
  readonly container: CorePluginHostContainer | null;
  readonly zone: Zone | null;
  readonly title: string | null;
  readonly titleKey: string | null;
  readonly icon: string | null;
  readonly order: number | null;
  readonly defaultVisible: boolean | null;
  readonly viewContainerId: string | null;
  readonly activityBar: {
    readonly title: string;
    readonly icon: string;
    readonly order: number | null;
  } | null;
  readonly label: string | null;
  readonly labelKey: string | null;
  readonly commandId: string | null;
  readonly group: string | null;
  readonly when: string | null;
  readonly anchor: CorePluginContextAnchor | null;
  readonly ui: {
    readonly kind: CorePluginUiSurfaceKind;
    readonly entrypoint: string;
    readonly bridgeVersion: string;
  } | null;
}

export interface CorePluginPermissionSet {
  readonly zone: Zone;
  readonly permissions: readonly CorePluginPermission[];
}

export interface CorePluginManifest {
  readonly id: string;
  readonly version: string;
  readonly hostApi: string;
  readonly runtime: CorePluginRuntime;
  readonly launch: { readonly program: string; readonly args: readonly string[] } | null;
  readonly targetApp: { readonly appId: string; readonly api: string } | null;
  readonly contributions: readonly CorePluginContribution[];
  readonly permissionSets: readonly CorePluginPermissionSet[];
}

export interface CorePluginSummary {
  readonly id: string;
  readonly version: string;
  readonly active: boolean;
  readonly state: PluginState;
}

export interface CorePluginInstallPreview {
  readonly manifest: CorePluginManifest;
  readonly existing: CorePluginSummary | null;
  readonly operation: 'install' | 'upgrade';
  readonly compatible: boolean;
  readonly canInstall: boolean;
  readonly issues: readonly string[];
}

export interface CorePluginProjection {
  readonly summary: CorePluginSummary;
  readonly targetApp: { readonly appId: string; readonly api: string } | null;
  readonly contributions: readonly CorePluginContribution[];
}

export interface CoreError {
  readonly code: string;
  readonly message: string;
  readonly retryable: boolean;
  readonly expectedRevision?: number;
  readonly actualRevision?: number;
}

export interface CoreTransport {
  request(request: CoreRequest): Promise<CoreResponse>;
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

export type CoreEvent =
  | { readonly kind: 'layout-changed'; readonly revision: number; readonly state: LayoutSnapshot }
  | { readonly kind: 'job-progress'; readonly jobId: string; readonly progress: number }
  | { readonly kind: 'plugin-disabled'; readonly extensionId: string; readonly reason: string };

export interface LayoutSnapshot {
  readonly schemaVersion: number;
  readonly revision: number;
  readonly activeApp: WorkbenchAppId;
  readonly parts: Record<Part, {
    readonly visible: boolean;
    readonly size: number;
    readonly collapsed: boolean;
  }>;
  readonly containers: Record<string, {
    readonly visible: boolean;
    readonly order: number;
  }>;
  readonly views: Record<string, {
    readonly visible: boolean;
    readonly order: number;
    readonly containerId?: string;
  }>;
}
