import type {
  CoreResponse,
  CoreTransport,
  LayoutCommand,
  LayoutOrderAnchor,
  LayoutSnapshot,
  Part,
  WorkbenchAppId,
  WorkspaceId,
} from '../protocol.js';
import { migrateLayoutState } from './layout-migrations.js';

export type LayoutListener = (snapshot: LayoutSnapshot) => void;

export class LayoutService {
  private snapshotValue: LayoutSnapshot | undefined;
  private readonly listeners = new Set<LayoutListener>();
  private readonly unsubscribe: () => void;

  public constructor(private readonly core: CoreTransport) {
    this.unsubscribe = core.subscribe((event) => {
      if (event.kind !== 'layout-changed') {
        return;
      }
      if (!this.snapshotValue || event.revision >= this.snapshotValue.revision) {
        this.setSnapshot(event.state);
      }
    });
  }

  public async load(): Promise<LayoutSnapshot> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'query-layout',
    });
    return this.setSnapshot(this.layoutFromResponse(response));
  }

  public snapshot(): LayoutSnapshot | undefined {
    return this.snapshotValue;
  }

  public subscribe(listener: LayoutListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public async activateApp(appId: WorkbenchAppId): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.activate-app', appId });
  }

  public async activateWorkspace(workspace: WorkspaceId): Promise<LayoutSnapshot> {
    return this.activateApp(workspace);
  }

  public async setPartVisibility(part: Part, visible: boolean): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.set-part-visibility', part, visible });
  }

  public async resizePart(part: Part, size: number): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.resize-part', part, size });
  }

  public async setPartCollapsed(part: Part, collapsed: boolean): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.collapse-part', part, collapsed });
  }

  public async setViewContainerVisibility(
    containerId: string,
    visible: boolean,
  ): Promise<LayoutSnapshot> {
    return this.apply({
      id: 'workbench.set-view-container-visibility',
      containerId,
      visible,
    });
  }

  public async setViewVisibility(
    viewId: string,
    visible: boolean,
  ): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.set-view-visibility', viewId, visible });
  }

  public async upsertViewContainer(
    containerId: string,
    visible: boolean,
    order: number,
  ): Promise<LayoutSnapshot> {
    return this.apply({
      id: 'workbench.upsert-view-container',
      containerId,
      visible,
      order,
    });
  }

  public async reorderViewContainer(
    containerId: string,
    anchor?: LayoutOrderAnchor,
  ): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.reorder-view-container', containerId, anchor });
  }

  public async moveView(
    viewId: string,
    targetContainerId: string,
    anchor?: LayoutOrderAnchor,
  ): Promise<LayoutSnapshot> {
    return this.apply({
      id: 'workbench.move-view',
      viewId,
      targetContainerId,
      anchor,
    });
  }

  public async removeViewContainer(containerId: string): Promise<LayoutSnapshot> {
    return this.apply({ id: 'workbench.remove-view-container', containerId });
  }

  public async apply(command: LayoutCommand): Promise<LayoutSnapshot> {
    const response = await this.core.request({
      protocolVersion: 1,
      kind: 'layout-command',
      command,
      expectedRevision: this.snapshotValue?.revision,
    });
    return this.setSnapshot(this.layoutFromResponse(response));
  }

  public dispose(): void {
    this.unsubscribe();
    this.listeners.clear();
  }

  private setSnapshot(snapshot: LayoutSnapshot): LayoutSnapshot {
    const next = migrateLayoutState(snapshot);
    this.snapshotValue = next;
    for (const listener of this.listeners) {
      try {
        listener(next);
      } catch {
        // One renderer projection must not prevent other projections from updating.
      }
    }
    return next;
  }

  private layoutFromResponse(response: CoreResponse): LayoutSnapshot {
    if (response.kind !== 'layout') {
      throw new Error(`Expected a layout response, received ${response.kind}`);
    }

    return response.state;
  }
}
