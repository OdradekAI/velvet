import {
  createNativeTauriBridge,
  TauriShortcutAdapter,
  TauriCoreTransport,
  type TauriBridge,
  type TauriTransportOptions,
} from '../platform/index.js';
import type { CoreTransport, LayoutSnapshot } from '../platform/protocol.js';
import type {
  ActivationAwareCoreTransport,
  CoreActivationRequest,
} from '../platform/protocol.js';
import { Workbench } from '../workbench/workbench.js';

/** The desktop composition root owns startup ordering, not feature modules. */
export interface DesktopApp {
  readonly workbench: Workbench;
  subscribeActivation(listener: (request: CoreActivationRequest) => void): () => void;
  start(): Promise<LayoutSnapshot>;
  dispose(): Promise<void>;
}

/** Compose the framework-independent workbench with a platform transport. */
export function createDesktopApp(core: CoreTransport): DesktopApp {
  const workbench = new Workbench(core);
  const unsubscribeActivationTransport = isActivationAwareTransport(core)
    ? core.subscribeActivation((request) => workbench.acceptActivationRequest(request))
    : undefined;

  return {
    workbench,
    subscribeActivation(listener) {
      return workbench.subscribeActivation(listener);
    },
    async start() {
      await workbench.recovery.load();
      const layout = await workbench.layout.load();
      await workbench.localization.loadFromCore(core);
      return layout;
    },
    async dispose() {
      unsubscribeActivationTransport?.();
      workbench.dispose();
      workbench.layout.dispose();
      workbench.localization.dispose();
      workbench.jobs.dispose();
      workbench.recovery.dispose();
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
  let app: DesktopApp | undefined;
  const transport = new TauriCoreTransport(bridge, {
    ...options,
    onResync: async (snapshot) => {
      try {
        app?.workbench.applyAuthoritativeSnapshot(snapshot);
      } catch {
        // A renderer projection failure must not break the transport session.
      }
      try {
        await options?.onResync?.(snapshot);
      } catch {
        // Optional integration callbacks are isolated from core recovery.
      }
    },
  });
  app = createDesktopApp(transport);
  return app;
}

export function createNativeTauriDesktopApp(
  options?: TauriTransportOptions,
): DesktopApp {
  const app = createTauriDesktopApp(createNativeTauriBridge(), options);
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

function isActivationAwareTransport(
  core: CoreTransport,
): core is ActivationAwareCoreTransport {
  return 'subscribeActivation' in core && typeof core.subscribeActivation === 'function';
}
