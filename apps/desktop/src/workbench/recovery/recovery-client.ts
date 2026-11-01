import type {
  CoreRecoveryProjection,
  CoreTransport,
} from '../../platform/protocol.js';

/** Host-owned projection adapter for lifecycle recovery state. */
export class RecoveryClient {
  private projectionState: CoreRecoveryProjection | undefined;
  private readonly listeners = new Set<(projection: CoreRecoveryProjection) => void>();
  private readonly unsubscribeCore: () => void;

  public constructor(private readonly core: CoreTransport) {
    this.unsubscribeCore = core.subscribe((event) => {
      if (event.kind !== 'recovery-changed') {
        return;
      }
      this.accept(event.projection);
    });
  }

  public snapshot(): CoreRecoveryProjection | undefined {
    return this.projectionState;
  }

  /** Applies an authoritative Rust projection after a transport resync. */
  public acceptSnapshot(projection: CoreRecoveryProjection): void {
    this.accept(projection);
  }

  public subscribe(listener: (projection: CoreRecoveryProjection) => void): () => void {
    this.listeners.add(listener);
    if (this.projectionState) {
      listener(this.projectionState);
    }
    return () => this.listeners.delete(listener);
  }

  public async load(): Promise<CoreRecoveryProjection> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'query-recovery',
    });
    if (response.kind !== 'recovery') {
      throw new Error(`Expected a recovery response, received ${response.kind}`);
    }
    this.accept(response.projection);
    return response.projection;
  }

  public async exportDiagnostics(destination: string): Promise<void> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'export-recovery-diagnostics',
      destination,
    });
    if (response.kind !== 'accepted') {
      throw new Error(`Expected an accepted recovery export response, received ${response.kind}`);
    }
  }

  public async restoreRecoverySet(): Promise<void> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'restore-recovery-set',
    });
    if (response.kind !== 'accepted') {
      throw new Error(`Expected an accepted recovery restore response, received ${response.kind}`);
    }
  }

  public async quarantineFailure(): Promise<void> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'quarantine-recovery-failure',
    });
    if (response.kind !== 'accepted') {
      throw new Error(`Expected an accepted recovery quarantine response, received ${response.kind}`);
    }
  }

  public dispose(): void {
    this.unsubscribeCore();
    this.listeners.clear();
    this.projectionState = undefined;
  }

  private accept(projection: CoreRecoveryProjection): void {
    this.projectionState = projection;
    for (const listener of this.listeners) {
      listener(projection);
    }
  }
}
