import { save } from '@tauri-apps/plugin-dialog';

export interface FileExporter {
  pickExportPath(defaultPath?: string): Promise<string | undefined>;
}

/** Native destination selection for Rust-owned exports. */
export function createNativeFileExporter(): FileExporter {
  return {
    async pickExportPath(defaultPath = 'velvet-plugin-recovery-log.json') {
      const selected = await save({
        defaultPath,
        filters: [{ name: 'JSON', extensions: ['json'] }],
        title: 'Export plugin recovery log',
      });
      return selected ?? undefined;
    },
  };
}
