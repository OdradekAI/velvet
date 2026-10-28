/**
 * Transport-facing types shared by the desktop app and the Rust core.
 *
 * This module deliberately has no Vue, React, or Tauri dependency. A future
 * local-socket or remote adapter can implement the same protocol.
 */
export type WorkspaceId = 'asset-manager' | 'feed-workbench';
export type Zone = 'navigation' | 'contextual' | 'operations';
export type Part =
  | 'title-bar'
  | 'activity-bar'
  | 'primary-sidebar'
  | 'workspace-area'
  | 'context-sidebar'
  | 'operations-panel'
  | 'status-bar'
  | 'overlay-layer';

export type CoreRequest =
  | { kind: 'query-layout' }
  | { kind: 'activate-workspace'; workspace: WorkspaceId }
  | { kind: 'set-part-visibility'; part: Part; visible: boolean }
  | { kind: 'resize-part'; part: Part; size: number }
  | {
      kind: 'resolve-preview';
      assetId: string;
      contentKind: string;
      intent: 'quick' | 'inspect' | 'analyze';
      surface: 'workspace' | 'thumbnail' | 'inspector';
    };

export interface CoreTransport {
  request(request: CoreRequest): Promise<unknown>;
  subscribe(listener: (event: CoreEvent) => void): () => void;
}

export type CoreEvent =
  | { kind: 'layout-changed'; state: LayoutSnapshot }
  | { kind: 'job-progress'; jobId: string; progress: number };

export interface LayoutSnapshot {
  schemaVersion: number;
  activeWorkspace: WorkspaceId;
  parts: Record<Part, { visible: boolean; size: number }>;
  views: Record<string, { visible: boolean; order: number }>;
}
