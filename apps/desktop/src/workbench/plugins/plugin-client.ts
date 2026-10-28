import type { CoreTransport, Zone } from '../protocol.js';
import { canUsePermission } from '../zones/zone-policy.js';

export interface ViewContribution {
  id: string;
  title: string;
  zone: Zone;
  defaultVisible?: boolean;
}

export interface ExtensionManifest {
  id: string;
  version: string;
  hostApi: string;
  contributions: ViewContribution[];
  permissions: Partial<Record<Zone, string[]>>;
}

export class PluginClient {
  public constructor(private readonly _core: CoreTransport) {}

  public validateZone(manifest: ExtensionManifest): void {
    for (const contribution of manifest.contributions) {
      if (!['navigation', 'contextual', 'operations'].includes(contribution.zone)) {
        throw new Error(`Unsupported extension zone: ${contribution.zone}`);
      }

      for (const permission of manifest.permissions[contribution.zone] ?? []) {
        if (!canUsePermission(contribution.zone, permission)) {
          throw new Error(
            `Permission ${permission} is not allowed in zone ${contribution.zone}`,
          );
        }
      }
    }
  }
}
