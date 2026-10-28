import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { CommandExecutionContext } from '../workbench/commands/command-registry.js';
import { KeybindingRegistry } from '../workbench/commands/keybinding-registry.js';

export interface NativeShortcutEvent {
  readonly keybinding: string;
  readonly state: 'pressed' | 'released';
}

export interface TauriShortcutAdapterOptions {
  readonly eventName?: string;
  readonly context: () => CommandExecutionContext;
  readonly onError?: (error: unknown) => void;
}

/** Feeds native Tauri shortcut events into the host keybinding registry. */
export class TauriShortcutAdapter {
  private unlisten: UnlistenFn | undefined;

  public constructor(
    private readonly keybindings: KeybindingRegistry,
    private readonly options: TauriShortcutAdapterOptions,
  ) {}

  public async start(): Promise<void> {
    if (this.unlisten) {
      return;
    }

    this.unlisten = await listen<NativeShortcutEvent>(
      this.options.eventName ?? 'velvet_keybinding',
      (event) => {
        if (event.payload.state !== 'pressed') {
          return;
        }

        void this.keybindings
          .execute(event.payload.keybinding, undefined, this.options.context())
          .catch((error: unknown) => this.options.onError?.(error));
      },
    );
  }

  public async dispose(): Promise<void> {
    const unlisten = this.unlisten;
    this.unlisten = undefined;
    unlisten?.();
  }
}
