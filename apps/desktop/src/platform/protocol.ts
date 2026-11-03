/**
 * Canonical desktop/core contract. This module deliberately has no React,
 * Vue, Tauri or feature-module dependency.
 */
export const PROTOCOL_VERSION = 1 as const;
export type ProtocolVersion = typeof PROTOCOL_VERSION;
export const PROTOCOL_MAJOR = 1 as const;
export const PROTOCOL_MINOR = 0 as const;
export const TRANSPORT_FEATURE_SESSION_V1 = 'session-v1' as const;
export const TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1 = 'schema-negotiation-v1' as const;
export const PROFILE_SCHEMA_VERSION = 1 as const;
export type ProfileSchemaVersion = typeof PROFILE_SCHEMA_VERSION;
export const PLUGIN_RESOURCE_SCHEMA_VERSION = 1 as const;
export type PluginResourceSchemaVersion = typeof PLUGIN_RESOURCE_SCHEMA_VERSION;
export const RECOVERY_SCHEMA_VERSION = 1 as const;
export type RecoverySchemaVersion = typeof RECOVERY_SCHEMA_VERSION;
export const CORE_SNAPSHOT_SCHEMA_VERSION = 1 as const;
export const RECOVERY_DIAGNOSTIC_SCHEMA_VERSION = 1 as const;
export type RecoveryDiagnosticSchemaVersion = typeof RECOVERY_DIAGNOSTIC_SCHEMA_VERSION;
export const PLUGIN_DIAGNOSTIC_SCHEMA_VERSION = 1 as const;
export type PluginDiagnosticSchemaVersion = typeof PLUGIN_DIAGNOSTIC_SCHEMA_VERSION;
export const CRASH_CAPTURE_SCHEMA_VERSION = 1 as const;
export const ACTIVATION_SCHEMA_VERSION = 1 as const;
export type ActivationSchemaVersion = typeof ACTIVATION_SCHEMA_VERSION;
export const RELEASE_MANIFEST_SCHEMA_VERSION = 1 as const;
export const RELEASE_TRANSACTION_SCHEMA_VERSION = 1 as const;
export const JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION = 1 as const;
export type JobExecutionEnvironmentSchemaVersion =
  typeof JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION;
export const OPERATION_SCHEMA_VERSION = 1 as const;
export const PORTABLE_BUNDLE_SCHEMA_VERSION = 1 as const;
export const IMPORT_PLAN_SCHEMA_VERSION = 1 as const;
export const IMPORT_RECEIPT_SCHEMA_VERSION = 1 as const;
export const ASSET_BUNDLE_SCHEMA_VERSION = 1 as const;
export const IMPORT_QUARANTINE_SCHEMA_VERSION = 1 as const;
export const NATIVE_UPDATE_SCHEMA_VERSION = 1 as const;
export type WorkbenchAppId = string;

export type SourceAvailability = 'available' | 'degraded' | 'permission-denied' | 'unknown';

export interface PortableAssetRecord {
  readonly assetId: string;
  readonly contentKind: string;
  readonly fingerprint?: string;
  readonly sourceAvailability?: SourceAvailability;
}

export interface PortableAssetMetadata {
  readonly schemaVersion: typeof ASSET_BUNDLE_SCHEMA_VERSION;
  readonly assets: readonly PortableAssetRecord[];
}

export interface ImportQuarantineEntry {
  readonly schemaVersion: typeof IMPORT_QUARANTINE_SCHEMA_VERSION;
  readonly bundleId: string;
  readonly planId: string;
  readonly sectionId: string;
  readonly ownerId: string;
  readonly sectionSchemaVersion: number;
  readonly quarantinedAtMs: number;
  readonly reason: string;
  readonly payload: readonly number[];
}

export interface ImportQuarantineSummary {
  readonly schemaVersion: typeof IMPORT_QUARANTINE_SCHEMA_VERSION;
  readonly bundleId: string;
  readonly planId: string;
  readonly sectionId: string;
  readonly ownerId: string;
  readonly sectionSchemaVersion: number;
  readonly quarantinedAtMs: number;
  readonly reason: string;
  readonly payloadBytes: number;
}

export type CoreActivationTargetKind = 'file-path' | 'deep-link';

export interface CoreActivationTarget {
  readonly kind: CoreActivationTargetKind;
  /** `file-path` is canonicalized by the native host before delivery. */
  readonly value: string;
}

export interface CoreActivationRequest {
  readonly schemaVersion: ActivationSchemaVersion;
  readonly requestId: string;
  readonly targets: readonly CoreActivationTarget[];
}
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

export interface CorePreviewActionContext {
  readonly sessionId: string;
  readonly providerId: string;
  readonly assetId: string;
  readonly contentKind: string;
  readonly intent: 'quick' | 'inspect' | 'analyze';
  readonly surface: 'workspace' | 'thumbnail' | 'inspector';
}

export type CoreRequest =
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-layout' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-profile' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-recovery' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-release-status' }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'export-recovery-diagnostics';
      readonly destination: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'restore-recovery-set';
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'quarantine-recovery-failure';
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'read-plugin-localization';
      readonly extensionId: string;
      readonly locale: string;
    }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-extensions' }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-plugin-install-recoveries';
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-plugin-install-recovery-log';
    }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-jobs' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-snapshot' }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-request-outcome';
      readonly requestId: number;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-plugin-projection';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'query-plugin-storage-recovery';
      readonly extensionId: string;
    }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-plugin-runtime-recovery' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-import-quarantine' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-operation-receipts' }
  | { readonly protocolVersion: ProtocolVersion; readonly kind: 'query-approved-source-roots' }
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
      readonly kind: 'restore-plugin-state';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'quarantine-plugin-state';
      readonly extensionId: string;
      readonly generation: number;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'close-quarantined-runtime-lease';
      readonly extensionId: string;
      readonly runtimeNonce: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'discard-import-quarantine';
      readonly bundleId: string;
      readonly planId: string;
      readonly sectionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'commit-asset-delete';
      readonly plan: CoreOperationPlan;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'preview-asset-delete';
      readonly plan: CoreOperationPlan;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'install-plugin';
      readonly sourceDir: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'install-plugin-archive';
      readonly sourceArchive: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'inspect-plugin';
      readonly sourceDir: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'inspect-plugin-archive';
      readonly sourceArchive: string;
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
      readonly kind: 'resolve-plugin-install-recovery';
      readonly extensionId: string;
      readonly action: CorePluginInstallRecoveryAction;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'clear-plugin-install-recovery-log';
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'export-plugin-install-recovery-log';
      readonly destination: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'uninstall-plugin';
      readonly extensionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'register-preview-asset';
      readonly assetId: string;
      readonly sourcePath: string;
      readonly contentKind: string;
      /** Canonical directory boundary enforced by the Rust source owner. */
      readonly approvedRoot?: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'add-approved-source-root';
      readonly root: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'remove-approved-source-root';
      readonly root: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'remove-preview-asset';
      readonly assetId: string;
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
      readonly kind: 'invoke-preview-action';
      readonly extensionId: string;
      readonly commandId: string;
      readonly args: unknown;
      readonly context: CorePreviewActionContext;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'release-preview-session';
      readonly sessionId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'create-job';
      readonly jobKind: string;
      readonly durationMs: number;
      readonly willFail: boolean;
      readonly input?: unknown;
      readonly policy?: CoreJobExecutionPolicy;
      readonly budget?: CoreJobBudgetRequest;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'cancel-job';
      readonly jobId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'retry-job';
      readonly jobId: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'set-job-execution-environment';
      readonly environment: CoreJobExecutionEnvironment;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'set-locale';
      readonly locale: string;
      readonly expectedRevision?: number;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'set-plugin-developer-mode';
      readonly enabled: boolean;
      readonly expectedRevision?: number;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'stage-release';
      readonly transactionId: string;
      readonly manifest: CoreReleaseManifest;
      readonly stagedPackageId: string;
      readonly packageGeneration: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'commit-release-health';
      readonly transactionId: string;
      readonly checkpoint: string;
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'rollback-release';
      readonly transactionId: string;
      readonly reason: string;
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
    }
  | {
      readonly protocolVersion: ProtocolVersion;
      readonly kind: 'resolve-preview-asset';
      readonly assetId: string;
      readonly intent: 'quick' | 'inspect' | 'analyze';
      readonly surface: 'workspace' | 'thumbnail' | 'inspector';
    };

export type CoreResponse =
  | { readonly kind: 'layout'; readonly state: LayoutSnapshot }
  | { readonly kind: 'profile'; readonly state: CoreProfileState }
  | { readonly kind: 'recovery'; readonly projection: CoreRecoveryProjection }
  | { readonly kind: 'plugin-resource'; readonly resource: CorePluginResource }
  | { readonly kind: 'preview'; readonly preview: CoreResolvedPreview }
  | { readonly kind: 'extensions'; readonly extensions: readonly CorePluginSummary[] }
  | { readonly kind: 'extension'; readonly extension: CorePluginSummary }
  | { readonly kind: 'plugin-install-preview'; readonly preview: CorePluginInstallPreview }
  | {
      readonly kind: 'plugin-install-recoveries';
      readonly recoveries: readonly CorePluginInstallRecovery[];
    }
  | {
      readonly kind: 'plugin-install-recovery-log';
      readonly log: readonly CorePluginInstallRecoveryLogEntry[];
    }
  | { readonly kind: 'plugin-projection'; readonly projection: CorePluginProjection }
  | {
      readonly kind: 'plugin-storage-recovery';
      readonly recovery: CorePluginStorageRecovery;
    }
  | {
      readonly kind: 'plugin-runtime-recovery';
      readonly recoveries: readonly CorePluginRuntimeRecovery[];
    }
  | {
      readonly kind: 'import-quarantine';
      readonly entries: readonly ImportQuarantineSummary[];
    }
  | { readonly kind: 'operation-receipt'; readonly receipt: CoreOperationReceipt }
  | { readonly kind: 'operation-receipts'; readonly receipts: readonly CoreOperationReceipt[] }
  | { readonly kind: 'operation-plan'; readonly plan: CoreOperationPlan }
  | { readonly kind: 'approved-source-roots'; readonly roots: readonly string[] }
  | { readonly kind: 'job'; readonly job: CoreJobSnapshot }
  | { readonly kind: 'jobs'; readonly jobs: readonly CoreJobSnapshot[] }
  | { readonly kind: 'snapshot'; readonly snapshot: CoreAuthoritativeSnapshot }
  | { readonly kind: 'request-outcome'; readonly outcome: CoreRequestOutcome }
  | { readonly kind: 'release-status'; readonly status: CoreReleaseStatus }
  | { readonly kind: 'accepted' };

export type CoreRequestOutcomeStatus = 'unknown' | 'completed';

export interface CoreRequestOutcome {
  readonly requestId: number;
  readonly status: CoreRequestOutcomeStatus;
}

export interface CoreAuthoritativeSnapshot {
  readonly schemaVersion: number;
  readonly layout: LayoutSnapshot;
  readonly profile: CoreProfileState;
  readonly recovery: CoreRecoveryProjection;
  readonly jobs: readonly CoreJobSnapshot[];
  readonly extensions: readonly CorePluginSummary[];
}

export interface CorePreviewCandidate {
  readonly providerId: string;
  readonly contentKinds: readonly string[];
  readonly intents: readonly ('quick' | 'inspect' | 'analyze')[];
  readonly surfaces: readonly ('workspace' | 'thumbnail' | 'inspector')[];
  readonly priority: number;
}

export interface CorePreviewContext {
  readonly assetId: string;
  readonly contentKind: string;
  readonly intent: 'quick' | 'inspect' | 'analyze';
  readonly surface: 'workspace' | 'thumbnail' | 'inspector';
}

export interface CorePreviewSession {
  readonly providerId: string;
  readonly context: CorePreviewContext;
  readonly sessionId: string;
  readonly renderer:
    | { readonly kind: 'generic' }
    | { readonly kind: 'text'; readonly text: string; readonly truncated: boolean }
    | {
        readonly kind: 'image';
        readonly mimeType: string;
        readonly bytes: readonly number[];
        readonly truncated: boolean;
      }
    | { readonly kind: 'ui-bridge'; readonly entrypoint: string; readonly bridgeVersion: string };
  readonly commands: readonly string[];
  readonly slots: readonly string[];
}

export interface CoreResolvedPreview {
  readonly candidate: CorePreviewCandidate;
  readonly session: CorePreviewSession;
  readonly diagnostic: string | null;
  readonly augmentations: readonly CoreResolvedPreviewAugmentation[];
}

export interface CoreResolvedPreviewAugmentation {
  readonly augmentationId: string;
  readonly slot: string;
  readonly session: CorePreviewSession | null;
  readonly diagnostic: string | null;
}

export type CoreJobStatus =
  | 'queued'
  | 'running'
  | 'retrying'
  | 'cancelling'
  | 'paused'
  | 'interrupted'
  | 'cancelled'
  | 'failed'
  | 'completed';

export type CoreJobNetworkPolicy = 'local-only' | 'unmetered' | 'any-network';
export type CoreJobNetworkState = 'online' | 'metered' | 'offline' | 'unknown';
export type CoreStorageAvailability =
  | 'writable'
  | 'read-only'
  | 'full'
  | 'unavailable'
  | 'unknown';
export type CoreStorageErrorCode =
  | 'storage-read-only'
  | 'storage-full'
  | 'storage-unavailable'
  | 'storage-unknown';
export type CorePowerPressure = 'normal' | 'battery' | 'thermal' | 'critical' | 'unknown';

export interface CoreJobBudgetRequest {
  readonly memoryBytes: number;
  readonly diskBytes: number;
  readonly childProcesses: number;
  readonly concurrency: number;
  readonly cpuMillisPerMinute: number;
  readonly networkBytesPerMinute: number;
}

export interface CoreJobRetryPolicy {
  readonly maxAttempts: number;
  readonly initialBackoffMs: number;
  readonly maxBackoffMs: number;
}

export interface CoreJobExecutionPolicy {
  readonly deadlineMs: number | null;
  readonly cancellationGraceMs: number;
  readonly retry: CoreJobRetryPolicy;
  readonly network: CoreJobNetworkPolicy;
  readonly background: boolean;
}

export interface CoreJobExecutionEnvironment {
  readonly schemaVersion: JobExecutionEnvironmentSchemaVersion;
  readonly suspended: boolean;
  readonly network: CoreJobNetworkState;
  readonly storage: CoreStorageAvailability;
  readonly power: CorePowerPressure;
}

export interface CoreJobCheckpoint {
  readonly schemaVersion: number;
  readonly payload: unknown;
}

export interface CoreJobSnapshot {
  readonly id: string;
  readonly kind: string;
  readonly status: CoreJobStatus;
  readonly progress: number;
  readonly message: string | null;
  readonly attempt: number;
  readonly retryable: boolean;
  readonly durationMs: number;
  readonly willFail: boolean;
  readonly policy: CoreJobExecutionPolicy;
  readonly budget: CoreJobBudgetRequest;
  readonly checkpoint: CoreJobCheckpoint | null;
  readonly input: unknown;
  readonly provenance: CoreJobProvenance;
  readonly result: CoreJobResult | null;
}

export interface CoreJobProvenance {
  readonly ownerId: string;
  readonly assetId: string | null;
  readonly contentKind: string | null;
  readonly inputFingerprint: string | null;
  readonly providerId: string | null;
  readonly providerVersion: string | null;
  readonly settingsFingerprint: string | null;
  readonly schemaVersion: number;
}

export interface CoreJobResult {
  readonly schemaVersion: number;
  readonly contentType: string;
  readonly payload: unknown;
}

export type CoreStartupPhase =
  | 'preflight'
  | 'recovering-state'
  | 'core-ready'
  | 'workbench-ready'
  | 'optional-activation'
  | 'ready';
export type CoreRecoveryMode = 'starting' | 'safe' | 'degraded' | 'ready';
export type CoreRecoveryFailureClass =
  | 'abnormal-exit'
  | 'startup-timeout'
  | 'core-migration'
  | 'state-corrupt'
  | 'journal-unavailable'
  | 'optional-component'
  | 'platform-initialization'
  | 'release-rollback';
export type CoreRecoveryAction =
  | 'retry'
  | 'disable-recent-component'
  | 'restore-snapshot'
  | 'quarantine-state'
  | 'reset-optional-state'
  | 'export-diagnostics';

export interface CoreRecoveryTarget {
  readonly ownerId: string;
  readonly scope: string;
  readonly generation: number;
}

export interface CoreRecoveryFailure {
  readonly class: CoreRecoveryFailureClass;
  readonly phase: CoreStartupPhase;
  readonly message: string;
  readonly atMs: number;
  readonly target: CoreRecoveryTarget | null;
}

export interface CoreRecoveryCheckpoint {
  readonly attemptId: number;
  readonly phase: CoreStartupPhase;
  readonly atMs: number;
}

export type CoreDurabilityMode = 'atomic-rename-only' | 'require-fsync';

export interface CoreDurabilityReport {
  readonly schemaVersion: number;
  readonly mode: CoreDurabilityMode;
  readonly atomicReplace: boolean;
  readonly fileSync: boolean;
  readonly directorySync: boolean;
  readonly crashDurable: boolean;
  readonly detail: string;
}

export interface CoreRecoveryProjection {
  readonly schemaVersion: RecoverySchemaVersion;
  readonly mode: CoreRecoveryMode;
  readonly phase: CoreStartupPhase;
  readonly attemptId: number;
  readonly consecutiveFailures: number;
  readonly lastFailure: CoreRecoveryFailure | null;
  readonly lastGood: CoreRecoveryCheckpoint | null;
  readonly actions: readonly CoreRecoveryAction[];
  readonly platformPreflight?: readonly CorePlatformPreflightEvidence[];
  readonly durabilityReport?: CoreDurabilityReport | null;
  readonly storage?: CoreStorageAvailability | null;
}

export type CoreReleaseChannel = 'stable' | 'preview';
export type CoreReleaseTransactionState = 'staged' | 'committed' | 'rolled-back';

export interface CoreReleaseManifest {
  readonly schemaVersion: number;
  readonly channel: CoreReleaseChannel;
  readonly version: string;
  readonly target: string;
  readonly artifactSha256: string;
  readonly updaterSchema: number;
  readonly minDataSchema: number;
  readonly maxDataSchema: number;
  readonly minClientVersion: string;
  readonly releaseSequence: number;
  readonly issuedAtMs: number;
  readonly expiresAtMs: number;
  readonly signingKeyId: string;
  readonly signature: string;
}

export interface CoreReleaseTransaction {
  readonly schemaVersion: number;
  readonly transactionId: string;
  readonly manifest: CoreReleaseManifest;
  readonly stagedPackageId: string;
  readonly packageGeneration: string;
  readonly baseDataGeneration: number | null;
  readonly committedDataGeneration: number | null;
  readonly state: CoreReleaseTransactionState;
  readonly healthCheckpoint: string | null;
  readonly failureReason: string | null;
  readonly createdAtMs: number;
}

export interface CoreReleaseStatus {
  readonly schemaVersion: number;
  readonly committedSequence: number;
  readonly committedPackageGeneration: string;
  readonly pending: CoreReleaseTransaction | null;
  readonly lastTransaction: CoreReleaseTransaction | null;
}

export type CoreOperationKind = 'delete' | 'overwrite' | 'move' | 'batch-job' | 'remote';
export type CoreOperationErrorCode =
  | 'operation-invalid-plan'
  | 'operation-stale-target'
  | 'operation-already-recorded'
  | 'operation-reconciliation-required'
  | 'operation-provider-failed'
  | 'operation-provider-unknown'
  | 'operation-persistence';
export type CoreOperationRetryPolicy =
  | 'provider-declared-idempotent'
  | 'reconcile-before-retry'
  | 'never';
export type CoreOperationUndoCapability = 'none' | 'provider-token' | 'bounded-backup';
export type CoreOperationReceiptStatus =
  | 'started'
  | 'committed'
  | 'partial'
  | 'rolled-back'
  | 'cancelled'
  | 'unknown';

export interface CoreOperationTarget {
  readonly targetId: string;
  readonly expectedRevision?: number;
  readonly expectedFingerprint?: string;
}

export interface CoreOperationPlan {
  readonly schemaVersion: number;
  readonly planId: string;
  readonly operationId: string;
  readonly ownerId: string;
  readonly kind: CoreOperationKind;
  readonly targets: readonly CoreOperationTarget[];
  readonly createdAtMs: number;
  readonly expiresAtMs: number;
  readonly retryPolicy: CoreOperationRetryPolicy;
  readonly undo: CoreOperationUndoCapability;
}

export interface CoreOperationItemReceipt {
  readonly targetId: string;
  readonly status: CoreOperationReceiptStatus;
  readonly message?: string | null;
}

export interface CoreOperationReceipt {
  readonly schemaVersion: number;
  readonly planId: string;
  readonly operationId: string;
  readonly status: CoreOperationReceiptStatus;
  readonly reconciled: boolean;
  readonly recordedAtMs: number;
  readonly items: readonly CoreOperationItemReceipt[];
  readonly undoToken?: string | null;
}

export type CoreNativeUpdateState =
  | 'disabled'
  | 'no-update'
  | 'available'
  | 'installing'
  | 'restart-required'
  | 'failed';

export interface CoreNativeUpdateStatus {
  readonly schemaVersion: typeof NATIVE_UPDATE_SCHEMA_VERSION;
  readonly state: CoreNativeUpdateState;
  readonly channel: CoreReleaseChannel;
  readonly currentVersion: string;
  readonly version: string | null;
  readonly target: string | null;
  readonly releaseSequence: number | null;
  readonly transactionId: string | null;
  readonly notes: string | null;
  readonly message: string | null;
}

export interface CoreRecoveryDiagnosticBundle {
  readonly schemaVersion: RecoveryDiagnosticSchemaVersion;
  readonly exportedAtMs: number;
  readonly projection: CoreRecoveryProjection;
  readonly pluginDiagnostics: readonly CorePluginDiagnosticEvent[];
  readonly diagnosticsDegraded: boolean;
  readonly crashEvidence?: readonly CoreCrashCaptureEvidence[];
}

/** Compatibility alias for the combined lifecycle/plugin diagnostic export. */
export type CoreDiagnosticBundle = CoreRecoveryDiagnosticBundle;

export type CorePluginDiagnosticKind =
  | 'activation'
  | 'runtime'
  | 'invocation'
  | 'job'
  | 'storage'
  | 'migration'
  | 'shutdown';

export type CorePlatformCapability = 'webview' | 'gpu';
export type CorePlatformCapabilityStatus = 'available' | 'degraded' | 'unavailable' | 'unknown';

export interface CorePlatformPreflightEvidence {
  readonly capability: CorePlatformCapability;
  readonly status: CorePlatformCapabilityStatus;
  readonly detail: string;
  readonly checkedAtMs: number;
}

export type CoreCrashCaptureKind =
  | 'process-exit'
  | 'panic'
  | 'signal'
  | 'exception'
  | 'oom'
  | 'runtime';

export interface CoreCrashCaptureEvidence {
  readonly schemaVersion: number;
  readonly correlationId: string;
  readonly processId: number;
  readonly parentProcessId: number | null;
  readonly attemptId: number;
  readonly component: string;
  readonly kind: CoreCrashCaptureKind;
  readonly code: string;
  readonly message: string;
  readonly runtimeNonce: string | null;
  readonly logTail: string | null;
  readonly artifact: CoreCrashArtifactAttachment | null;
  readonly occurredAtMs: number;
}

export interface CoreCrashArtifactAttachment {
  readonly artifactId: string;
  readonly format: string;
  readonly sizeBytes: number;
  readonly sha256: string;
  readonly collectedAtMs: number;
  readonly metadata: CoreCrashArtifactMetadata | null;
}

export interface CoreCrashArtifactMetadata {
  readonly processName: string | null;
  readonly processId: number | null;
  readonly exceptionType: string | null;
  readonly signal: string | null;
  readonly reason: string | null;
}

export interface CorePluginDiagnosticEvent {
  readonly schemaVersion: PluginDiagnosticSchemaVersion;
  readonly sequence: number;
  readonly extensionId: string;
  readonly kind: CorePluginDiagnosticKind;
  readonly code: string;
  readonly message: string;
  readonly jobId: string | null;
  readonly attempt: number | null;
  readonly retryable: boolean;
  readonly occurredAtMs: number;
}

export type PluginState = 'registered' | 'starting' | 'active' | 'disabled' | 'failed';

export interface PluginSummary {
  readonly id: string;
  readonly version: string;
  readonly active: boolean;
  readonly state: PluginState;
}

export type CorePluginExtensionKind = 'ui' | 'workspace' | 'capability';
export type CorePluginActivationEvent =
  | { readonly kind: 'onWorkbenchReady' }
  | { readonly kind: 'onApp'; readonly appId: string }
  | { readonly kind: 'onView'; readonly contributionId: string }
  | { readonly kind: 'onCommand'; readonly contributionId: string }
  | { readonly kind: 'onPreview'; readonly selector: string }
  | { readonly kind: 'onJob'; readonly jobKind: string };

export interface CorePluginSupport {
  readonly trustedWorkspace: boolean;
  readonly virtualWorkspace: boolean;
  readonly profiles: boolean;
}

export interface CorePluginLocalization {
  readonly defaultLocale: string;
  readonly supportedLocales: readonly string[];
  readonly resourceRoot: string;
  readonly bundleSchema: number;
}

export interface CorePluginStorageDeclaration {
  readonly schemaVersion: number;
  readonly quotaBytes: number;
}

export interface CorePluginWorkbenchApp {
  readonly id: string;
  readonly api: string;
  readonly title: string;
  readonly activityBarIcon: string;
  readonly activityBarOrder: number | null;
  readonly surface: {
    readonly kind: CorePluginUiSurfaceKind;
    readonly entrypoint: string;
    readonly bridgeVersion: string;
  };
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
  | 'app-surface'
  | 'activity-bar-entry'
  | 'companion-view'
  | 'operation-view'
  | 'view-container'
  | 'view'
  | 'dialog'
  | 'context-menu'
  | 'menu'
  | 'keybinding'
  | 'popover'
  | 'notification'
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
  readonly preview: CorePreviewContribution | null;
}

export type CorePreviewContributionIntent = 'quick' | 'inspect' | 'analyze';
export type CorePreviewContributionSurface = 'workspace' | 'thumbnail' | 'inspector';
export type CorePreviewContributionSlot =
  | 'preview-overlay'
  | 'inspector'
  | 'metadata'
  | 'histogram'
  | 'toolbar-action'
  | 'context-menu'
  | 'analysis-action';

export interface CorePreviewContribution {
  readonly contentKinds: readonly string[];
  readonly intents: readonly CorePreviewContributionIntent[];
  readonly surfaces: readonly CorePreviewContributionSurface[];
  readonly priority: number;
  readonly slot: CorePreviewContributionSlot | null;
}

export type CorePluginManifestContribution = Omit<
  CorePluginContribution,
  'namespace'
> & {
  readonly namespace?: string | null;
};

export interface CorePluginPermissionSet {
  readonly zone: Zone;
  readonly permissions: readonly CorePluginPermission[];
}

export interface CorePluginManifest {
  readonly [key: `x-${string}`]: unknown;
  readonly manifestSchema: number;
  readonly id: string;
  readonly version: string;
  readonly hostApi: string;
  readonly bridgeApi: string;
  readonly runtimeProtocol: string;
  readonly runtime: CorePluginRuntime;
  readonly extensionKind: readonly CorePluginExtensionKind[];
  readonly launch: { readonly program: string; readonly args: readonly string[] } | null;
  readonly activationEvents: readonly CorePluginActivationEvent[];
  readonly extensionDependencies: readonly string[];
  readonly targetApp: { readonly appId: string; readonly api: string } | null;
  readonly support: CorePluginSupport;
  readonly requiredCapabilities: readonly string[];
  readonly optionalCapabilities: readonly string[];
  readonly localization: CorePluginLocalization | null;
  readonly storage: CorePluginStorageDeclaration | null;
  readonly workbenchApp: CorePluginWorkbenchApp | null;
  readonly contributions: readonly CorePluginManifestContribution[];
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

export type CorePluginInstallRecoveryKind = 'staging' | 'rollback';
export type CorePluginInstallRecoveryAction = 'resume' | 'commit' | 'rollback' | 'discard';
export type CorePluginInstallRecoveryLogStatus = 'succeeded' | 'failed';

export interface CorePluginInstallRecovery {
  readonly extensionId: string;
  readonly kind: CorePluginInstallRecoveryKind;
  readonly operation: 'install' | 'upgrade';
  readonly candidateVersion: string | null;
  readonly current: CorePluginSummary | null;
  readonly rollbackVersion: string | null;
  readonly canResume: boolean;
  readonly canCommit: boolean;
  readonly canRollback: boolean;
  readonly canDiscard: boolean;
  readonly issues: readonly string[];
}

export interface CorePluginInstallRecoveryLogEntry {
  readonly extensionId: string;
  readonly action: CorePluginInstallRecoveryAction;
  readonly status: CorePluginInstallRecoveryLogStatus;
  readonly attempt: number;
  readonly message: string;
  readonly removedPaths: readonly string[];
  readonly occurredAtMs: number;
}

export interface CorePluginProjection {
  readonly summary: CorePluginSummary;
  readonly manifest: CorePluginManifest;
}

export type CorePluginStorageRecoveryStatus = 'available' | 'empty' | 'corrupt' | 'unavailable';

export interface CorePluginStorageRecovery {
  readonly extensionId: string;
  readonly status: CorePluginStorageRecoveryStatus;
  readonly currentGeneration: number | null;
  readonly previousGeneration: number | null;
  readonly schemaVersion: number | null;
  readonly previousSchemaVersion: number | null;
  readonly quarantineGeneration: number | null;
  readonly reason: string | null;
}

export type CorePluginRuntimeRecoveryState = 'quarantined';

export interface CorePluginRuntimeRecovery {
  readonly extensionId: string;
  readonly runtimeNonce: string;
  readonly state: CorePluginRuntimeRecoveryState;
  readonly startedAtMs: number;
}

export interface CoreError {
  readonly code: string;
  readonly message: string;
  readonly retryable: boolean;
  readonly expectedRevision?: number;
  readonly actualRevision?: number;
}

export interface CoreProfileState {
  readonly schemaVersion: ProfileSchemaVersion;
  readonly revision: number;
  readonly locale: string;
  /** Explicit Profile authorization for unsigned local extension packages. */
  readonly pluginDeveloperMode?: boolean;
}

export interface CorePluginResource {
  readonly schemaVersion: PluginResourceSchemaVersion;
  readonly extensionId: string;
  readonly locale: string;
  readonly contentType: string;
  readonly bytes: readonly number[];
}

export interface CoreTransport {
  request(request: CoreRequest): Promise<CoreResponse>;
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

export type CoreTransportState =
  | 'disconnected'
  | 'connecting'
  | 'connected'
  | 'resyncing'
  | 'degraded';

export interface CoreTransportStatus {
  readonly state: CoreTransportState;
  readonly reason?: string;
}

export interface ProtocolHandshakeRequest {
  readonly clientId: string;
  readonly protocolMajor: number;
  readonly protocolMinor: number;
  readonly features: readonly string[];
  readonly schemaVersions: Readonly<Record<string, number>>;
}

export interface ProtocolHandshakeResponse {
  readonly protocolMajor: number;
  readonly protocolMinor: number;
  readonly selectedFeatures: readonly string[];
  readonly schemaVersions: Readonly<Record<string, number>>;
  readonly sessionId: string;
}

export interface SessionAwareCoreTransport extends CoreTransport {
  connect(): Promise<ProtocolHandshakeResponse>;
  reconnect(): Promise<ProtocolHandshakeResponse>;
  resync(): Promise<CoreAuthoritativeSnapshot>;
  queryRequestOutcome(requestId: number): Promise<CoreRequestOutcome>;
  subscribeStatus(listener: (status: CoreTransportStatus) => void): () => void;
}

export interface ActivationAwareCoreTransport extends CoreTransport {
  subscribeActivation(listener: (request: CoreActivationRequest) => void): () => void;
}

export type CoreEvent =
  | { readonly kind: 'layout-changed'; readonly revision: number; readonly state: LayoutSnapshot }
  | { readonly kind: 'profile-changed'; readonly revision: number; readonly state: CoreProfileState }
  | { readonly kind: 'recovery-changed'; readonly projection: CoreRecoveryProjection }
  | { readonly kind: 'plugin-diagnostic'; readonly diagnostic: CorePluginDiagnosticEvent }
  | { readonly kind: 'job-changed'; readonly snapshot: CoreJobSnapshot }
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
