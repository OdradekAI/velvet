import type { CoreTransport, LayoutSnapshot, Part, WorkspaceId } from '../protocol.js';
import { migrateLayoutState } from './layout-migrations.js';

export class LayoutService {
  private snapshotValue: LayoutSnapshot | undefined;

  public constructor(private readonly core: CoreTransport) {}

  public async load(): Promise<LayoutSnapshot> {
    this.snapshotValue = migrateLayoutState(
      (await this.core.request({ kind: 'query-layout' })) as LayoutSnapshot,
    );
    return this.snapshotValue;
  }

  public snapshot(): LayoutSnapshot | undefined {
    return this.snapshotValue;
  }

  public async activateWorkspace(workspace: WorkspaceId): Promise<void> {
    await this.core.request({ kind: 'activate-workspace', workspace });
  }

  public async setPartVisibility(part: Part, visible: boolean): Promise<void> {
    await this.core.request({ kind: 'set-part-visibility', part, visible });
  }

  public async resizePart(part: Part, size: number): Promise<void> {
    await this.core.request({ kind: 'resize-part', part, size });
  }
}
