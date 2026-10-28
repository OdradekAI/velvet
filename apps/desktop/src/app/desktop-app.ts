import {
  createNativeTauriTransport,
  TauriShortcutAdapter,
  TauriCoreTransport,
  type TauriBridge,
  type TauriTransportOptions,
} from '../platform/index.js';
import type { CoreTransport, LayoutSnapshot } from '../platform/protocol.js';
import { Workbench } from '../workbench/workbench.js';

/** The desktop composition root owns startup ordering, not feature modules. */
export interface DesktopApp {
  readonly workbench: Workbench;
  start(): Promise<LayoutSnapshot>;
  dispose(): Promise<void>;
}

/** Compose the framework-independent workbench with a platform transport. */
export function createDesktopApp(core: CoreTransport): DesktopApp {
  const workbench = new Workbench(core);

  return {
    workbench,
    async start() {
      return workbench.layout.load();
    },
    async dispose() {
      workbench.layout.dispose();
      if (isDisposableTransport(core)) {
        await core.dispose();
      }
    },
  };
}

/** Compose the desktop app with the native Tauri invoke/listen bridge. */
export function createTauriDesktopApp(
  bridge: TauriBridge,
  options?: TauriTransportOptions,
): DesktopApp {
  return createDesktopApp(new TauriCoreTransport(bridge, options));
}

export function createNativeTauriDesktopApp(
  options?: TauriTransportOptions,
): DesktopApp {
  const app = createDesktopApp(createNativeTauriTransport(options));
  const shortcuts = new TauriShortcutAdapter(app.workbench.keybindings, {
    context: () => ({
      activeAppId: app.workbench.context.snapshot().activeAppId,
      keys: app.workbench.contextKeys.snapshot(),
    }),
    onError: (error) => console.error('Native Workbench shortcut failed', error),
  });

  return {
    ...app,
    async start() {
      const layout = await app.start();
      await shortcuts.start();
      return layout;
    },
    async dispose() {
      await shortcuts.dispose();
      await app.dispose();
    },
  };
}

function isDisposableTransport(
  core: CoreTransport,
): core is CoreTransport & { dispose: () => Promise<void> } {
  return 'dispose' in core && typeof core.dispose === 'function';
}
