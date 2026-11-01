import { open, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';

export interface SourceRootPicker {
  pick(): Promise<string | undefined>;
}

type OpenDialog = (options: OpenDialogOptions) => Promise<string | string[] | null>;

/** Native directory selection for the host-owned source-root policy. */
export function createNativeSourceRootPicker(openDialog: OpenDialog = open): SourceRootPicker {
  return {
    async pick() {
      const selected = await openDialog({
        directory: true,
        multiple: false,
        title: 'Choose an approved Velvet source folder',
      });
      return typeof selected === 'string' ? selected : undefined;
    },
  };
}
