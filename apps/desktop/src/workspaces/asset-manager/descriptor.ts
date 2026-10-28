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
};
