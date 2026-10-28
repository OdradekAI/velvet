import type { LayoutSnapshot, Part, WorkbenchAppId } from '../protocol.js';
import { PARTS } from '../parts/part-registry.js';

export const CURRENT_LAYOUT_SCHEMA = 1;

export function defaultLayoutState(): LayoutSnapshot {
  const parts = {} as LayoutSnapshot['parts'];
  const ids: Part[] = [
    'title-bar',
    'activity-bar',
    'primary-sidebar',
    'workspace-area',
    'auxiliary-sidebar',
    'operations-panel',
    'status-bar',
    'overlay-layer',
  ];

  for (const id of ids) {
    const descriptor = PARTS.find((part) => part.id === id);
    parts[id] = {
      visible: true,
      size: descriptor?.defaultSize ?? 0,
      collapsed: false,
    };
  }

  return {
    schemaVersion: CURRENT_LAYOUT_SCHEMA,
    revision: 0,
    activeApp: 'asset-manager' satisfies WorkbenchAppId,
    containers: {},
    parts,
    views: {},
  };
}
