import type { LayoutSnapshot, Part, WorkspaceId } from '../protocol.js';

export const CURRENT_LAYOUT_SCHEMA = 1;

export function defaultLayoutState(): LayoutSnapshot {
  const parts = {} as LayoutSnapshot['parts'];
  const ids: Part[] = [
    'title-bar',
    'activity-bar',
    'primary-sidebar',
    'workspace-area',
    'context-sidebar',
    'operations-panel',
    'status-bar',
    'overlay-layer',
  ];

  for (const id of ids) {
    parts[id] = { visible: true, size: 0 };
  }

  return {
    schemaVersion: CURRENT_LAYOUT_SCHEMA,
    activeWorkspace: 'asset-manager' satisfies WorkspaceId,
    parts,
    views: {},
  };
}
