import type { WorkspaceDescriptor } from '../index.js';

export const ASSET_MANAGER_WORKSPACE: WorkspaceDescriptor = {
  id: 'asset-manager',
  apiVersion: '1',
  title: 'Resource Manager',
  activityBarIcon: 'library',
  surface: {
    trust: 'trusted',
    entrypoint: 'builtin://resource-manager',
  },
  viewContainers: [
    {
      id: 'asset-manager.navigation',
      title: 'Library',
      icon: 'library',
      zone: 'navigation',
      preferredPart: 'primary-sidebar',
      scope: { kind: 'workbench-app', appId: 'asset-manager' },
      order: 0,
      defaultVisible: true,
      activityBar: { title: 'Library', icon: 'library', order: 0 },
    },
  ],
  views: [
    {
      id: 'asset-manager.asset-library',
      containerId: 'asset-manager.navigation',
      title: 'Assets',
      icon: 'grid',
      zone: 'navigation',
      scope: { kind: 'workbench-app', appId: 'asset-manager' },
      order: 0,
      defaultVisible: true,
      activation: 'eager',
    },
  ],
};
