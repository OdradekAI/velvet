import type { WorkspaceId } from '../platform/protocol.js';
import type { WorkbenchAppDescriptor } from '../workbench/apps/workbench-app.js';
import { ASSET_MANAGER_WORKSPACE } from './asset-manager/descriptor.js';
import { FEED_WORKBENCH_WORKSPACE } from './feed-workbench/descriptor.js';

export type WorkspaceDescriptor = Omit<WorkbenchAppDescriptor, 'id'> & {
  id: WorkspaceId;
};

/** Product workspaces are stable Activity Bar entries, not plugin views. */
export const BUILTIN_WORKBENCH_APPS: readonly WorkspaceDescriptor[] = [
  ASSET_MANAGER_WORKSPACE,
  FEED_WORKBENCH_WORKSPACE,
];

/** Compatibility name for existing renderer imports. */
export const BUILTIN_WORKSPACES = BUILTIN_WORKBENCH_APPS;
