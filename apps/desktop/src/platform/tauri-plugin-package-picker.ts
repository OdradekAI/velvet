import { open, type OpenDialogOptions } from '@tauri-apps/plugin-dialog';

export type PluginPackageSelection = 'directory' | 'archive';

export interface PluginPackagePicker {
  pick(selection: PluginPackageSelection): Promise<string | undefined>;
}

type OpenDialog = (options: OpenDialogOptions) => Promise<string | string[] | null>;

/** Native selection for directory and ZIP plugin packages. */
export function createNativePluginPackagePicker(openDialog: OpenDialog = open): PluginPackagePicker {
  return {
    async pick(selection) {
      const options: OpenDialogOptions = selection === 'directory'
        ? {
            directory: true,
            multiple: false,
            title: 'Select a Velvet plugin directory package',
          }
        : {
            directory: false,
            multiple: false,
            filters: [{ name: 'Velvet plugin package', extensions: ['zip'] }],
            title: 'Select a Velvet plugin ZIP package',
          };
      const selected = await openDialog(options);
      return typeof selected === 'string' ? selected : undefined;
    },
  };
}
