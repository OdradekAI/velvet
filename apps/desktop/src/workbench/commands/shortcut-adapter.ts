import type {
  NativeShortcutEvent,
  NativeShortcutEventListener,
} from '../../platform/tauri-shortcut-adapter.js';
import { KeybindingRegistry } from './keybinding-registry.js';
import type { CommandExecutionContext } from './command-registry.js';

export interface ShortcutEventSource {
  start(listener: NativeShortcutEventListener): Promise<void>;
  dispose(): Promise<void>;
}

export interface WorkbenchShortcutAdapterOptions {
  readonly context: () => CommandExecutionContext;
  readonly onError?: (error: unknown) => void;
}

/** Bridges native shortcut events into the Workbench-owned keybinding registry. */
export class WorkbenchShortcutAdapter {
  private started = false;

  public constructor(
    private readonly keybindings: KeybindingRegistry,
    private readonly source: ShortcutEventSource,
    private readonly options: WorkbenchShortcutAdapterOptions,
  ) {}

  public async start(): Promise<void> {
    if (this.started) {
      return;
    }

    this.started = true;
    try {
      await this.source.start((event) => this.handle(event));
    } catch (error: unknown) {
      this.started = false;
      throw error;
    }
  }

  public async dispose(): Promise<void> {
    if (!this.started) {
      return;
    }

    this.started = false;
    await this.source.dispose();
  }

  private handle(event: NativeShortcutEvent): void {
    if (event.state !== 'pressed') {
      return;
    }

    let context: CommandExecutionContext;
    try {
      context = this.options.context();
    } catch (error: unknown) {
      this.reportError(error);
      return;
    }

    void this.keybindings
      .execute(event.keybinding, undefined, context)
      .catch((error: unknown) => this.reportError(error));
  }

  private reportError(error: unknown): void {
    try {
      this.options.onError?.(error);
    } catch {
      // Error reporting must not escape the native event callback.
    }
  }
}
