import type {
  CoreJobSnapshot,
  CoreResponse,
  CoreTransport,
} from '../../platform/protocol.js';

export interface CreateJobOptions {
  readonly kind: string;
  readonly durationMs: number;
  readonly willFail?: boolean;
}

/** Renderer adapter for the Rust-owned JobSystem projection. */
export class JobClient {
  private readonly listeners = new Set<() => void>();
  private readonly jobs = new Map<string, CoreJobSnapshot>();
  private readonly unsubscribeTransport: () => void;

  public constructor(private readonly core: CoreTransport) {
    this.unsubscribeTransport = core.subscribe((event) => {
      if (event.kind !== 'job-changed') {
        return;
      }
      this.jobs.set(event.snapshot.id, event.snapshot);
      this.emit();
    });
  }

  public list(): readonly CoreJobSnapshot[] {
    return [...this.jobs.values()].sort((left, right) => left.id.localeCompare(right.id));
  }

  public get(id: string): CoreJobSnapshot | undefined {
    return this.jobs.get(id);
  }

  /** Records a job returned by a command/action handoff before its first event. */
  public accept(snapshot: CoreJobSnapshot): void {
    this.jobs.set(snapshot.id, snapshot);
    this.emit();
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public async query(): Promise<readonly CoreJobSnapshot[]> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'query-jobs',
    });
    if (response.kind !== 'jobs') {
      throw new Error(`Expected jobs response, received ${response.kind}`);
    }
    this.jobs.clear();
    for (const job of response.jobs) {
      this.jobs.set(job.id, job);
    }
    this.emit();
    return this.list();
  }

  public async create(options: CreateJobOptions): Promise<CoreJobSnapshot> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'create-job',
      jobKind: options.kind,
      durationMs: options.durationMs,
      willFail: options.willFail ?? false,
    });
    return this.acceptJobResponse(response);
  }

  public async cancel(id: string): Promise<CoreJobSnapshot> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'cancel-job',
      jobId: id,
    });
    return this.acceptJobResponse(response);
  }

  public async retry(id: string): Promise<CoreJobSnapshot> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'retry-job',
      jobId: id,
    });
    return this.acceptJobResponse(response);
  }

  public dispose(): void {
    this.unsubscribeTransport();
    this.listeners.clear();
  }

  private acceptJobResponse(response: CoreResponse): CoreJobSnapshot {
    if (response.kind !== 'job') {
      throw new Error(`Expected job response, received ${response.kind}`);
    }
    this.jobs.set(response.job.id, response.job);
    this.emit();
    return response.job;
  }

  private emit(): void {
    for (const listener of this.listeners) {
      listener();
    }
  }
}
