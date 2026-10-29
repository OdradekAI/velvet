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
  viewContainers: [
    {
      id: 'feed-workbench.navigation',
      title: 'Sources',
      icon: 'inbox',
      zone: 'navigation',
      preferredPart: 'primary-sidebar',
      scope: { kind: 'workbench-app', appId: 'feed-workbench' },
      order: 0,
      defaultVisible: true,
      activityBar: { title: 'Sources', icon: 'inbox', order: 0 },
    },
  ],
  views: [
    {
      id: 'feed-workbench.feed-sources',
      containerId: 'feed-workbench.navigation',
      title: 'Subscriptions',
      icon: 'rss',
      zone: 'navigation',
      scope: { kind: 'workbench-app', appId: 'feed-workbench' },
      order: 0,
      defaultVisible: true,
      activation: 'eager',
    },
  ],
};
