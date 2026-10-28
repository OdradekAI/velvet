import type { WorkbenchAppId } from '../../platform/protocol.js';

export type ContextSelectionKind =
  | 'asset'
  | 'feed-item'
  | 'collection'
  | 'source';

export interface ContextSelectionRef {
  readonly kind: ContextSelectionKind;
  readonly id: string;
}

export type DataScopeKind = 'workspace' | 'library' | 'collection' | 'source';

export interface DataScopeRef {
  readonly kind: DataScopeKind;
  readonly id: string;
}

export interface ContextSort {
  readonly field: string;
  readonly direction: 'ascending' | 'descending';
}

export interface HandoffIntent {
  readonly kind: 'open' | 'inspect' | 'materialize' | 'process';
  readonly target?: ContextSelectionRef;
  readonly targetAppId?: WorkbenchAppId;
  readonly sourceAppId?: WorkbenchAppId;
}

export type ContextFilterValue = string | number | boolean | readonly string[];

/** Read-only projection shared by WorkbenchApps. */
export interface WorkspaceContextSnapshot {
  readonly revision: number;
  readonly activeAppId: WorkbenchAppId;
  readonly dataScope: DataScopeRef | null;
  readonly selection: readonly ContextSelectionRef[];
  readonly query: string;
  readonly filters: Readonly<Record<string, ContextFilterValue>>;
  readonly sort: readonly ContextSort[];
  readonly handoff: HandoffIntent | null;
}

export interface WorkspaceContext {
  snapshot(): WorkspaceContextSnapshot;
  subscribe(listener: (snapshot: WorkspaceContextSnapshot) => void): () => void;
}

export type WorkspaceContextCommand =
  | { readonly kind: 'set-data-scope'; readonly scope: DataScopeRef | null }
  | {
      readonly kind: 'set-selection';
      readonly selection: readonly ContextSelectionRef[];
    }
  | { readonly kind: 'set-query'; readonly query: string }
  | {
      readonly kind: 'set-filters';
      readonly filters: Readonly<Record<string, ContextFilterValue>>;
    }
  | { readonly kind: 'set-sort'; readonly sort: readonly ContextSort[] }
  | { readonly kind: 'request-handoff'; readonly intent: HandoffIntent };

export type WorkspaceContextHostCommand =
  | WorkspaceContextCommand
  | { readonly kind: 'activate-app'; readonly appId: WorkbenchAppId };

export class InMemoryWorkspaceContext implements WorkspaceContext {
  private current: WorkspaceContextSnapshot;
  private readonly listeners = new Set<
    (snapshot: WorkspaceContextSnapshot) => void
  >();

  public constructor(activeAppId: WorkbenchAppId = 'asset-manager') {
    this.current = {
      revision: 0,
      activeAppId,
      dataScope: null,
      selection: [],
      query: '',
      filters: {},
      sort: [],
      handoff: null,
    };
  }

  public snapshot(): WorkspaceContextSnapshot {
    return this.current;
  }

  public subscribe(listener: (snapshot: WorkspaceContextSnapshot) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  /** Host-only mutation seam; app surfaces receive the read-only projection. */
  public apply(command: WorkspaceContextHostCommand): WorkspaceContextSnapshot {
    switch (command.kind) {
      case 'activate-app':
        this.current = { ...this.current, activeAppId: command.appId };
        break;
      case 'set-data-scope':
        this.current = { ...this.current, dataScope: command.scope };
        break;
      case 'set-selection':
        this.current = { ...this.current, selection: [...command.selection] };
        break;
      case 'set-query':
        this.current = { ...this.current, query: command.query };
        break;
      case 'set-filters':
        this.current = { ...this.current, filters: { ...command.filters } };
        break;
      case 'set-sort':
        this.current = { ...this.current, sort: [...command.sort] };
        break;
      case 'request-handoff':
        this.current = { ...this.current, handoff: command.intent };
        break;
    }

    this.current = { ...this.current, revision: this.current.revision + 1 };
    for (const listener of this.listeners) {
      listener(this.current);
    }

    return this.current;
  }
}
