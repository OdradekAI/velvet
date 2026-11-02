import type { CoreNativeUpdateStatus } from './protocol.js';

export interface NativeUpdateBridge {
  invoke<T>(command: string, args?: Readonly<Record<string, unknown>>): Promise<T>;
}

/**
 * Renderer-facing seam for the host-owned updater coordinator. The renderer
 * never receives updater permissions or package bytes; it only observes the
 * typed status and asks the native host to perform a checked install.
 */
export class TauriUpdateCoordinator {
  private readonly bridge: NativeUpdateBridge;

  public constructor(bridge: NativeUpdateBridge) {
    this.bridge = bridge;
  }

  public check(): Promise<CoreNativeUpdateStatus> {
    return this.bridge.invoke<CoreNativeUpdateStatus>('velvet_update_check');
  }

  public install(expectedVersion: string): Promise<CoreNativeUpdateStatus> {
    return this.bridge.invoke<CoreNativeUpdateStatus>('velvet_update_install', {
      request: { expectedVersion },
    });
  }
}
