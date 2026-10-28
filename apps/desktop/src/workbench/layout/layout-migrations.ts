import { CURRENT_LAYOUT_SCHEMA, defaultLayoutState } from './layout-state.js';
import type { LayoutSnapshot } from '../protocol.js';

export function migrateLayoutState(input: LayoutSnapshot | undefined): LayoutSnapshot {
  if (!input) {
    return defaultLayoutState();
  }

  if (input.schemaVersion === CURRENT_LAYOUT_SCHEMA) {
    return input;
  }

  // Keep migrations explicit and monotonic. Add one branch per schema version.
  return {
    ...defaultLayoutState(),
    ...input,
    schemaVersion: CURRENT_LAYOUT_SCHEMA,
  };
}
