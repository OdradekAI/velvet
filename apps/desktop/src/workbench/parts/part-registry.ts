import type { Part } from '../protocol.js';

export interface PartDescriptor {
  id: Part;
  title: string;
  zone?: 'navigation' | 'contextual' | 'operations';
  movable: false;
  hideable: boolean;
  collapsible: boolean;
  resizable: boolean;
  minSize: number;
  maxSize: number;
  defaultSize: number;
}

export const PARTS: readonly PartDescriptor[] = [
  {
    id: 'title-bar',
    title: 'Title Bar',
    movable: false,
    hideable: true,
    collapsible: false,
    resizable: false,
    minSize: 36,
    maxSize: 36,
    defaultSize: 36,
  },
  {
    id: 'activity-bar',
    title: 'Activity Bar',
    zone: 'navigation',
    movable: false,
    hideable: true,
    collapsible: false,
    resizable: false,
    minSize: 48,
    maxSize: 48,
    defaultSize: 48,
  },
  {
    id: 'primary-sidebar',
    title: 'Primary Sidebar',
    zone: 'navigation',
    movable: false,
    hideable: true,
    collapsible: true,
    resizable: true,
    minSize: 180,
    maxSize: 520,
    defaultSize: 260,
  },
  {
    id: 'workspace-area',
    title: 'Workspace Area',
    movable: false,
    hideable: false,
    collapsible: false,
    resizable: false,
    minSize: 0,
    maxSize: Number.MAX_SAFE_INTEGER,
    defaultSize: 0,
  },
  {
    id: 'auxiliary-sidebar',
    title: 'Auxiliary Sidebar',
    zone: 'contextual',
    movable: false,
    hideable: true,
    collapsible: true,
    resizable: true,
    minSize: 220,
    maxSize: 520,
    defaultSize: 300,
  },
  {
    id: 'operations-panel',
    title: 'Operations Panel',
    zone: 'operations',
    movable: false,
    hideable: true,
    collapsible: true,
    resizable: true,
    minSize: 120,
    maxSize: 520,
    defaultSize: 180,
  },
  {
    id: 'status-bar',
    title: 'Status Bar',
    movable: false,
    hideable: true,
    collapsible: false,
    resizable: false,
    minSize: 24,
    maxSize: 24,
    defaultSize: 24,
  },
  {
    id: 'overlay-layer',
    title: 'Overlay Layer',
    movable: false,
    hideable: false,
    collapsible: false,
    resizable: false,
    minSize: 0,
    maxSize: 0,
    defaultSize: 0,
  },
];
