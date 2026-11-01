import type { CoreTransport } from '../protocol.js';

export type SourceRootListener = (roots: readonly string[]) => void;

/**
 * Host-owned view of the Rust source-root allow-list.
 *
 * The service is deliberately separate from workspace and plugin clients:
 * source roots are user security policy, not plugin state. A load failure is
 * observable by callers but never prevents the Workbench from starting.
 */
export class SourceRootService {
  private rootsValue: readonly string[] = [];
  private readonly listeners = new Set<SourceRootListener>();

  public constructor(private readonly core: CoreTransport) {}

  public roots(): readonly string[] {
    return this.rootsValue;
  }

  public subscribe(listener: SourceRootListener): () => void {
    this.listeners.add(listener);
    try {
      listener(this.rootsValue);
    } catch {
      // A settings consumer may fail while the service is still usable.
    }
    return () => this.listeners.delete(listener);
  }

  public async load(): Promise<readonly string[]> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'query-approved-source-roots',
    });
    if (response.kind !== 'approved-source-roots') {
      throw new Error(`Expected approved source roots, received ${response.kind}`);
    }
    this.setRoots(response.roots);
    return this.rootsValue;
  }

  public async add(root: string): Promise<readonly string[]> {
    await this.mutate({
      protocolVersion: 1,
      kind: 'add-approved-source-root',
      root: validateRootInput(root),
    });
    return this.load();
  }

  public async remove(root: string): Promise<readonly string[]> {
    await this.mutate({
      protocolVersion: 1,
      kind: 'remove-approved-source-root',
      root: validateRootInput(root),
    });
    return this.load();
  }

  private async mutate(
    request: Extract<
      Parameters<CoreTransport['request']>[0],
      { kind: 'add-approved-source-root' | 'remove-approved-source-root' }
    >,
  ): Promise<void> {
    const response = await this.core.request(request);
    if (response.kind !== 'accepted') {
      throw new Error(`Expected an accepted source-root response, received ${response.kind}`);
    }
  }

  private setRoots(roots: readonly string[]): void {
    this.rootsValue = roots.map(validateRootInput);
    for (const listener of this.listeners) {
      try {
        listener(this.rootsValue);
      } catch {
        // One settings consumer must not block source policy refresh.
      }
    }
  }
}

function validateRootInput(root: string): string {
  if (root.length === 0 || root.length > 4096 || root.includes('\0')) {
    throw new Error('Approved source root must be a bounded non-empty path');
  }
  return root;
}
