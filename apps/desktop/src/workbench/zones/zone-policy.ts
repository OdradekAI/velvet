import type { Zone } from '../protocol.js';

export const ZONE_POLICY: Record<Zone, {
  title: string;
  allowedPermissions: readonly string[];
}> = {
  navigation: {
    title: 'Navigation',
    allowedPermissions: ['workspace.open', 'library.read', 'source.connect'],
  },
  contextual: {
    title: 'Contextual tools',
    allowedPermissions: ['asset.read', 'metadata.read', 'metadata.write', 'preview.render'],
  },
  operations: {
    title: 'Operations',
    allowedPermissions: ['jobs.create', 'jobs.read', 'logs.read', 'external-process', 'network'],
  },
};

export function canUsePermission(zone: Zone, permission: string): boolean {
  return ZONE_POLICY[zone].allowedPermissions.includes(permission);
}
