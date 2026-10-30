import { save } from '@tauri-apps/plugin-dialog';

export interface FileExporter {
  pickExportPath(): Promise<string | undefined>;
}

/** Native destination selection for Rust-owned exports. */
export function createNativeFileExporter(): FileExporter {
  return {
    async pickExportPath() {
      const selected = await save({
        defaultPath: 'velvet-plugin-recovery-log.json',
        filters: [{ name: 'JSON', extensions: ['json'] }],
        title: 'Export plugin recovery log',
      });
      return selected ?? undefined;
    },
  };
}
