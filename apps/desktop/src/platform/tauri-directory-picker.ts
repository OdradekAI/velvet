import { open } from '@tauri-apps/plugin-dialog';

export interface DirectoryPicker {
  pickDirectory(): Promise<string | undefined>;
}

/** Native directory selection for local plugin packages. */
export function createNativeDirectoryPicker(): DirectoryPicker {
  return {
    async pickDirectory() {
      const selected = await open({
        directory: true,
        multiple: false,
        title: 'Select a Velvet plugin package',
      });
      return typeof selected === 'string' ? selected : undefined;
    },
  };
}
