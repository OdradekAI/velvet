import type { WorkspaceDescriptor } from '../index.js';

export const FEED_WORKBENCH_WORKSPACE: WorkspaceDescriptor = {
  id: 'feed-workbench',
  apiVersion: '1',
  title: 'Feed Workbench',
  activityBarIcon: 'inbox',
  surface: {
    trust: 'trusted',
    entrypoint: 'builtin://feed-workbench',
  },
};
