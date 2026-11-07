export const UI_COMPONENT_CONTRACT_SCHEMA = 'velvet.ui.component-contract' as const;
export const UI_COMPONENT_CONTRACT_SCHEMA_VERSION = 1 as const;

export type UiKitLayer = 'primitives' | 'overlays' | 'host-projections';
export type UiKitQualificationTier = 'node' | 'browser' | 'both';
export type ComponentEvidenceKind = 'observed' | 'inferred' | 'host-guarantee' | 'unknown';
export type UiKitAsyncLifecycle = 'success' | 'cancelled' | 'timeout' | 'failure' | 'retry';

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
  readonly asyncLifecycle?: readonly UiKitAsyncLifecycle[];
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
