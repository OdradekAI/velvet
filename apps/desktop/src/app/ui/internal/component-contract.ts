export const UI_COMPONENT_CONTRACT_SCHEMA = 'velvet.ui.component-contract' as const;
export const UI_COMPONENT_CONTRACT_SCHEMA_VERSION = 1 as const;

export type UiKitLayer = 'primitives' | 'overlays' | 'host-projections';
export type UiKitQualificationTier = 'node' | 'browser' | 'both';
export type ComponentEvidenceKind = 'observed' | 'inferred' | 'host-guarantee' | 'unknown';
export type UiKitAsyncLifecycle = 'success' | 'cancelled' | 'timeout' | 'failure' | 'retry';
export type UiKitActionTransition = 'requested' | 'committed' | 'cancelled' | 'failed' | 'timed-out' | 'retrying';
export type UiKitVisualSurface = 'canvas' | 'panel' | 'raised' | 'overlay' | 'active';
export type UiKitVisualRadius = 'sm' | 'md' | 'lg' | 'pill';
export type UiKitFocusModel = 'none' | 'roving' | 'trap' | 'restore' | 'pointer';
export type UiKitStateOwner = 'host' | 'component' | 'adapter';

export interface ComponentVisualContract {
  /** Stable semantic style profile; CSS classes and framework data attributes stay private. */
  readonly profile: 'compact-control' | 'dense-panel' | 'gallery' | 'preview' | 'overlay' | 'navigation';
  readonly surfaces: Readonly<{
    readonly root: UiKitVisualSurface;
    readonly interactive?: UiKitVisualSurface;
    readonly selected?: UiKitVisualSurface;
    readonly overlay?: UiKitVisualSurface;
  }>;
  readonly radius: UiKitVisualRadius;
  readonly density: 'compact' | 'default' | 'comfortable';
  /** Named geometry invariants, for example `thumbnail:4/3` or `control:28px`. */
  readonly geometry?: readonly string[];
}

export interface ComponentActionLifecycle<Action extends string = string> {
  readonly action: Action;
  readonly transitions: readonly UiKitActionTransition[];
}

export interface ComponentBehaviorContract<Action extends string = string> {
  readonly stateOwner: UiKitStateOwner;
  readonly focus: UiKitFocusModel;
  readonly selection: 'none' | 'host' | 'component';
  readonly actionLifecycle: readonly ComponentActionLifecycle<Action>[];
}

export interface ComponentEvidence {
  readonly kind: ComponentEvidenceKind;
  readonly reference: string;
  readonly claim: string;
}

export interface ComponentContract<
  Props extends object = object,
  State extends string = string,
  Action extends string = string,
  Observation extends string = string,
> {
  readonly schema: typeof UI_COMPONENT_CONTRACT_SCHEMA;
  readonly schemaVersion: typeof UI_COMPONENT_CONTRACT_SCHEMA_VERSION;
  readonly id: string;
  readonly layer: UiKitLayer;
  readonly qualification?: UiKitQualificationTier;
  readonly owner: 'component' | 'host' | 'adapter';
  readonly props: readonly (keyof Props | string)[];
  readonly slots: readonly string[];
  readonly states: readonly State[];
  readonly actions: readonly Action[];
  readonly observables: readonly Observation[];
  readonly browserActions?: readonly Action[];
  readonly asyncLifecycle?: readonly UiKitAsyncLifecycle[];
  readonly visual?: ComponentVisualContract;
  readonly behavior?: ComponentBehaviorContract<Action>;
  readonly stories: readonly string[];
  readonly documentation: string;
  readonly evidence: readonly ComponentEvidence[];
}

export function defineComponentContract<
  Props extends object,
  State extends string,
  Action extends string,
  Observation extends string,
>(contract: ComponentContract<Props, State, Action, Observation>): ComponentContract<Props, State, Action, Observation> {
  return contract;
}
