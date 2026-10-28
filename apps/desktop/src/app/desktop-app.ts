import {
  TauriCoreTransport,
  type TauriBridge,
  type TauriTransportOptions,
} from '../platform/index.js';
import type { CoreTransport } from '../platform/protocol.js';
import { Workbench } from '../workbench/workbench.js';

/** The desktop composition root owns startup ordering, not feature modules. */
export interface DesktopApp {
  readonly workbench: Workbench;
  start(): Promise<void>;
}

/** Compose the framework-independent workbench with a platform transport. */
export function createDesktopApp(core: CoreTransport): DesktopApp {
  const workbench = new Workbench(core);

  return {
    workbench,
    async start() {
      await workbench.layout.load();
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
