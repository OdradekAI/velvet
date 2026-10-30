import type { Part, Zone } from '../../platform/protocol.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';

/** Serialized isolated surface metadata supplied by an extension manifest. */
export interface PluginUiContribution {
  readonly kind: 'app-surface' | 'view' | 'dialog' | 'popover' | 'inspector';
  readonly entrypoint: string;
  readonly bridgeVersion: string;
}

export interface ContributionActivityBarEntry {
  readonly title: string;
  readonly icon: string;
  readonly order?: number;
}

export type ContributionScope = 'workbench' | 'workbench-app' | 'capability';

export type HostContainer =
  | Part
  | 'context-menu'
  | 'command-palette';

export type ContributionKind =
  | 'activity-bar-entry'
  | 'title-bar-action'
  | 'command'
  | 'keybinding'
  | 'menu'
  | 'context-menu'
  | 'dialog'
  | 'popover'
  | 'notification'
  | 'settings'
  | 'companion-view'
  | 'operation-view'
  | 'app-surface'
  | 'view-container'
  | 'view'
  | 'app-toolbar'
  | 'app-context-menu'
  | 'inspector-section'
  | 'source-connector'
  | 'feed-monitor'
  | 'content-kind-provider'
  | 'preview-provider'
  | 'preview-augmentation'
  | 'processor'
  | 'job'
  | 'asset-facet'
  | 'extension-model';

export type ContextAnchor =
  | 'asset'
  | 'feed-item'
  | 'collection'
  | 'selection'
  | 'view'
  | 'background';

export interface HostContainerDescriptor {
  readonly id: HostContainer;
  readonly title: string;
  readonly part?: Part;
  readonly global: boolean;
}

export const HOST_CONTAINERS: readonly HostContainerDescriptor[] = [
  { id: 'activity-bar', title: 'Activity Bar', part: 'activity-bar', global: true },
  { id: 'title-bar', title: 'Title Bar', part: 'title-bar', global: true },
  { id: 'workspace-area', title: 'Workspace Area', part: 'workspace-area', global: false },
  { id: 'primary-sidebar', title: 'Primary Sidebar', part: 'primary-sidebar', global: false },
  {
    id: 'auxiliary-sidebar',
    title: 'Auxiliary Sidebar',
    part: 'auxiliary-sidebar',
    global: true,
  },
  { id: 'operations-panel', title: 'Operations Panel', part: 'operations-panel', global: true },
  { id: 'status-bar', title: 'Status Bar', part: 'status-bar', global: true },
  { id: 'overlay-layer', title: 'Overlay Layer', part: 'overlay-layer', global: true },
  { id: 'context-menu', title: 'Context Menu', global: true },
  { id: 'command-palette', title: 'Command Palette', global: true },
];

export interface ContributionDescriptor {
  readonly id: string;
  readonly extensionId: string;
  readonly namespace: string;
  readonly scope: ContributionScope;
  readonly kind: ContributionKind;
  readonly appId?: WorkbenchAppId;
  readonly container?: HostContainer;
  readonly zone?: Zone;
  readonly anchor?: ContextAnchor;
  readonly order?: number;
  readonly permissions?: readonly string[];
  /** Display metadata for ViewContainer/View contributions. */
  readonly title?: string;
  readonly titleKey?: string;
  readonly label?: string;
  readonly labelKey?: string;
  readonly icon?: string;
  readonly defaultVisible?: boolean;
  readonly enablement?: string;
  readonly activation?: 'eager' | 'on-demand';
  readonly requiredCapabilities?: readonly string[];
  /** Local contribution id of the ViewContainer owned by this extension. */
  readonly viewContainerId?: string;
  readonly activityBar?: ContributionActivityBarEntry;
  readonly commandId?: string;
  readonly group?: string;
  readonly when?: string;
  /** Optional isolated UI surface bound to this contribution. */
  readonly ui?: PluginUiContribution;
  readonly preview?: PreviewContributionDescriptor;
}

export type PreviewContributionIntent = 'quick' | 'inspect' | 'analyze';
export type PreviewContributionSurface = 'workspace' | 'thumbnail' | 'inspector';
export type PreviewContributionSlot =
  | 'preview-overlay'
  | 'inspector'
  | 'metadata'
  | 'histogram'
  | 'toolbar-action'
  | 'context-menu'
  | 'analysis-action';

export interface PreviewContributionDescriptor {
  readonly contentKinds: readonly string[];
  readonly intents: readonly PreviewContributionIntent[];
  readonly surfaces: readonly PreviewContributionSurface[];
  readonly priority: number;
  readonly slot?: PreviewContributionSlot;
}

export interface ContributionQuery {
  readonly activeAppId?: WorkbenchAppId;
  readonly scope?: ContributionScope;
  readonly kind?: ContributionKind;
}

export class ContributionValidationError extends Error {
  public constructor(message: string) {
    super(message);
    this.name = 'ContributionValidationError';
  }
}

const WORKBENCH_KINDS: readonly ContributionKind[] = [
  'activity-bar-entry',
  'title-bar-action',
  'command',
  'keybinding',
  'context-menu',
  'dialog',
  'notification',
  'settings',
  'companion-view',
  'operation-view',
];

const WORKBENCH_APP_KINDS: readonly ContributionKind[] = [
  'app-surface',
  'view-container',
  'view',
  'app-toolbar',
  'app-context-menu',
  'inspector-section',
  'command',
  'keybinding',
  'settings',
  'context-menu',
  'dialog',
];

const CAPABILITY_KINDS: readonly ContributionKind[] = [
  'source-connector',
  'feed-monitor',
  'content-kind-provider',
  'preview-provider',
  'preview-augmentation',
  'processor',
  'job',
  'asset-facet',
  'extension-model',
];

const GLOBAL_ONLY_CONTAINERS: readonly HostContainer[] = [
  'auxiliary-sidebar',
  'operations-panel',
];

const STABLE_ID = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

export class ContributionRegistry {
  private readonly contributions = new Map<string, ContributionDescriptor>();

  public register(contribution: ContributionDescriptor): void {
    this.validate(contribution);
    const key = this.keyFor(contribution);
    if (this.contributions.has(key)) {
      throw new ContributionValidationError(`Duplicate contribution: ${key}`);
    }

    this.contributions.set(key, contribution);
  }

  public unregister(contribution: Pick<ContributionDescriptor, 'id' | 'extensionId' | 'scope' | 'appId'>): boolean {
    return this.contributions.delete(this.keyFor(contribution));
  }

  public get(
    contribution: Pick<ContributionDescriptor, 'id' | 'extensionId' | 'scope' | 'appId'>,
  ): ContributionDescriptor | undefined {
    return this.contributions.get(this.keyFor(contribution));
  }

  /** Pass activeAppId when resolving the contributions visible in one app. */
  public list(query: ContributionQuery = {}): readonly ContributionDescriptor[] {
    return [...this.contributions.values()]
      .filter((contribution) => {
        if (query.scope && contribution.scope !== query.scope) {
          return false;
        }
        if (query.kind && contribution.kind !== query.kind) {
          return false;
        }
        return (
          contribution.scope !== 'workbench-app' ||
          query.activeAppId === undefined ||
          contribution.appId === query.activeAppId
        );
      })
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  }

  private validate(contribution: ContributionDescriptor): void {
    if (!STABLE_ID.test(contribution.extensionId) || !STABLE_ID.test(contribution.id)) {
      throw new ContributionValidationError(
        `Contribution ids must use stable letters, numbers, dots, underscores or hyphens`,
      );
    }

    const expectedNamespace = namespaceFor(
      contribution.scope,
      contribution.appId,
      contribution.extensionId,
    );
    if (contribution.namespace !== expectedNamespace) {
      throw new ContributionValidationError(
        `Contribution ${contribution.id} must use namespace ${expectedNamespace}`,
      );
    }

    const allowedKinds = allowedKindsFor(contribution.scope);
    if (!allowedKinds.includes(contribution.kind)) {
      throw new ContributionValidationError(
        `${contribution.kind} is not valid in ${contribution.scope} scope`,
      );
    }

    if (contribution.scope === 'workbench-app' && !contribution.appId) {
      throw new ContributionValidationError(
        `App contribution ${contribution.id} must declare an app id`,
      );
    }

    if (
      contribution.scope === 'capability' &&
      (contribution.appId || contribution.container || contribution.zone)
    ) {
      throw new ContributionValidationError(
        `Capability ${contribution.id} cannot target an app, zone or host container`,
      );
    }

    if (
      (contribution.kind === 'view-container' || contribution.kind === 'view') &&
      !contribution.zone
    ) {
      throw new ContributionValidationError(
        `${contribution.kind} ${contribution.id} must declare a semantic zone`,
      );
    }

    if (
      (contribution.kind === 'context-menu' || contribution.kind === 'app-context-menu') &&
      !contribution.anchor
    ) {
      throw new ContributionValidationError(
        `${contribution.kind} ${contribution.id} must declare a context anchor`,
      );
    }

    if (contribution.container && !HOST_CONTAINERS.some((container) => container.id === contribution.container)) {
      throw new ContributionValidationError(
        `Unknown host container: ${contribution.container}`,
      );
    }

    if (contribution.scope === 'workbench-app' && contribution.container && GLOBAL_ONLY_CONTAINERS.includes(contribution.container)) {
      throw new ContributionValidationError(
        `${contribution.container} is reserved for global Workbench contributions`,
      );
    }

    this.validateKindContainer(contribution);
  }

  private validateKindContainer(contribution: ContributionDescriptor): void {
    const { kind, container } = contribution;
    const requiredContainer: Partial<Record<ContributionKind, HostContainer>> = {
      'activity-bar-entry': 'activity-bar',
      'title-bar-action': 'title-bar',
      'context-menu': 'context-menu',
      'app-context-menu': 'context-menu',
      dialog: 'overlay-layer',
      notification: 'overlay-layer',
      'companion-view': 'auxiliary-sidebar',
      'operation-view': 'operations-panel',
      'app-surface': 'workspace-area',
      'app-toolbar': 'workspace-area',
      'inspector-section': 'workspace-area',
    };

    const required = requiredContainer[kind] ?? viewHostContainer(contribution);
    if (required && container !== required) {
      throw new ContributionValidationError(
        `${kind} must target ${required}`,
      );
    }

    if (kind === 'companion-view' && contribution.scope !== 'workbench') {
      throw new ContributionValidationError('CompanionView is Workbench-global');
    }

    if (kind === 'operation-view' && contribution.scope !== 'workbench') {
      throw new ContributionValidationError('OperationView is Workbench-global');
    }
  }

  private keyFor(
    contribution: Pick<ContributionDescriptor, 'id' | 'extensionId' | 'scope' | 'appId'>,
  ): string {
    return `${namespaceFor(contribution.scope, contribution.appId, contribution.extensionId)}:${contribution.extensionId}:${contribution.id}`;
  }
}

function viewHostContainer(contribution: ContributionDescriptor): HostContainer | undefined {
  if (contribution.kind !== 'view-container' && contribution.kind !== 'view') {
    return undefined;
  }

  if (contribution.scope === 'workbench') {
    return contribution.zone === 'contextual'
      ? 'auxiliary-sidebar'
      : contribution.zone === 'operations'
        ? 'operations-panel'
        : 'primary-sidebar';
  }

  return contribution.zone === 'navigation' ? 'primary-sidebar' : 'workspace-area';
}

export function namespaceFor(
  scope: ContributionScope,
  appId: WorkbenchAppId | undefined,
  extensionId: string,
): string {
  switch (scope) {
    case 'workbench':
      return 'workbench';
    case 'workbench-app':
      if (!appId) {
        throw new ContributionValidationError(
          `WorkbenchApp namespace requires an app id for ${extensionId}`,
        );
      }
      return `workbench.app.${appId}`;
    case 'capability':
      return `capability.${extensionId}`;
  }
}

function allowedKindsFor(scope: ContributionScope): readonly ContributionKind[] {
  switch (scope) {
    case 'workbench':
      return WORKBENCH_KINDS;
    case 'workbench-app':
      return WORKBENCH_APP_KINDS;
    case 'capability':
      return CAPABILITY_KINDS;
  }
}
