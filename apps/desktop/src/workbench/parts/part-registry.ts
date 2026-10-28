import type { Part } from '../protocol.js';

export interface PartDescriptor {
  id: Part;
  title: string;
  zone?: 'navigation' | 'contextual' | 'operations';
  movable: false;
}

export const PARTS: readonly PartDescriptor[] = [
  { id: 'title-bar', title: 'Title Bar', movable: false },
  { id: 'activity-bar', title: 'Activity Bar', zone: 'navigation', movable: false },
  { id: 'primary-sidebar', title: 'Primary Sidebar', zone: 'navigation', movable: false },
  { id: 'workspace-area', title: 'Workspace Area', movable: false },
  { id: 'context-sidebar', title: 'Context Sidebar', zone: 'contextual', movable: false },
  { id: 'operations-panel', title: 'Operations Panel', zone: 'operations', movable: false },
  { id: 'status-bar', title: 'Status Bar', movable: false },
  { id: 'overlay-layer', title: 'Overlay Layer', movable: false },
];
