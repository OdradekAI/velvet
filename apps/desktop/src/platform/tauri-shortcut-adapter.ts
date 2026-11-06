import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export interface NativeShortcutEvent {
  readonly keybinding: string;
  readonly state: 'pressed' | 'released';
}

export interface TauriShortcutEventSourceOptions {
  readonly eventName?: string;
}

export type NativeShortcutEventListener = (event: NativeShortcutEvent) => void;

/** Provides native shortcut events without depending on Workbench command state. */
export class TauriShortcutEventSource {
  private unlisten: UnlistenFn | undefined;

  public constructor(
    private readonly options: TauriShortcutEventSourceOptions = {},
  ) {}

  public async start(listener: NativeShortcutEventListener): Promise<void> {
    if (this.unlisten) {
      return;
    }

    this.unlisten = await listen<NativeShortcutEvent>(
      this.options.eventName ?? 'velvet_keybinding',
      (event) => {
        listener(event.payload);
      },
    );
  }

  public async dispose(): Promise<void> {
    const unlisten = this.unlisten;
    this.unlisten = undefined;
    unlisten?.();
  }
}
