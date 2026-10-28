import type { WorkspaceId } from '../platform/protocol.js';
import { ASSET_MANAGER_WORKSPACE } from './asset-manager/descriptor.js';
import { FEED_WORKBENCH_WORKSPACE } from './feed-workbench/descriptor.js';

export interface WorkspaceDescriptor {
  id: WorkspaceId;
  title: string;
  activityBarIcon: string;
}

/** Product workspaces are stable Activity Bar entries, not plugin views. */
export const BUILTIN_WORKSPACES: readonly WorkspaceDescriptor[] = [
  ASSET_MANAGER_WORKSPACE,
  FEED_WORKBENCH_WORKSPACE,
];
