import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { TauriCoreTransport, type TauriBridge, type TauriTransportOptions } from './tauri-transport.js';

export function createNativeTauriBridge(): TauriBridge {
  return { invoke, listen };
}

export function createNativeTauriTransport(
  options?: TauriTransportOptions,
): TauriCoreTransport {
  return new TauriCoreTransport(createNativeTauriBridge(), options);
}
