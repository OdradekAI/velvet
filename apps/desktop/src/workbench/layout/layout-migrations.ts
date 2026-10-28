import { CURRENT_LAYOUT_SCHEMA, defaultLayoutState } from './layout-state.js';
import type { LayoutSnapshot } from '../protocol.js';
import { PARTS } from '../parts/part-registry.js';

export function migrateLayoutState(input: LayoutSnapshot | undefined): LayoutSnapshot {
  if (!input) {
    return defaultLayoutState();
  }

  const legacy = input as LayoutSnapshot & {
    readonly activeWorkspace?: string;
  };
  const activeApp = input.activeApp ?? legacy.activeWorkspace ?? 'asset-manager';
  const parts = Object.fromEntries(
    Object.entries(input.parts).map(([part, state]) => [
      part,
      {
        ...state,
        size:
          state.size ||
          PARTS.find((descriptor) => descriptor.id === part)?.defaultSize ||
          0,
        collapsed: state.collapsed ?? false,
      },
    ]),
  ) as LayoutSnapshot['parts'];
  const views = Object.fromEntries(
    Object.entries(input.views).map(([viewId, state]) => [viewId, { ...state }]),
  ) as LayoutSnapshot['views'];
  const containers = Object.fromEntries(
    Object.entries(input.containers ?? {}).map(([containerId, state]) => [
      containerId,
      { ...state },
    ]),
  ) as LayoutSnapshot['containers'];

  if (input.schemaVersion === CURRENT_LAYOUT_SCHEMA && input.activeApp) {
    return {
      ...input,
      parts,
      containers,
      views,
      activeApp,
      revision: input.revision ?? 0,
    };
  }

  // Keep migrations explicit and monotonic. Add one branch per schema version.
  return {
    ...defaultLayoutState(),
    ...input,
    activeApp,
    revision: input.revision ?? 0,
    parts,
    containers,
    views,
    schemaVersion: CURRENT_LAYOUT_SCHEMA,
  };
}
