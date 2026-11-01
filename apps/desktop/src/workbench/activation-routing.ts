import type {
  CoreActivationRequest,
  CoreActivationTarget,
  WorkspaceId,
} from './protocol.js';

export type WorkspaceRoutingIntent =
  | {
      readonly kind: 'open-file';
      readonly workspaceId: WorkspaceId;
      readonly requestId: string;
      readonly targetIndex: number;
      readonly path: string;
    }
  | {
      readonly kind: 'open-asset';
      readonly workspaceId: WorkspaceId;
      readonly requestId: string;
      readonly targetIndex: number;
      readonly assetId: string;
    }
  | {
      readonly kind: 'unrouted-deep-link';
      readonly requestId: string;
      readonly targetIndex: number;
      readonly value: string;
      readonly reason: 'unsupported-route';
    };

const ASSET_MANAGER_WORKSPACE = 'asset-manager' as const;

/**
 * Maps already validated native activation targets to host-owned workspace
 * intent. This function has no filesystem, plugin or command access and does
 * not mutate layout; a workspace decides whether and when to consume the
 * intent.
 */
export function routeActivationRequest(
  request: CoreActivationRequest,
): readonly WorkspaceRoutingIntent[] {
  return request.targets.map((target, targetIndex) =>
    routeActivationTarget(request.requestId, target, targetIndex),
  );
}

export function workspaceRoutingIntentKey(intent: WorkspaceRoutingIntent): string {
  switch (intent.kind) {
    case 'open-file':
      return `${intent.requestId}:${intent.targetIndex}:${intent.kind}:${intent.workspaceId}:${intent.path}`;
    case 'open-asset':
      return `${intent.requestId}:${intent.targetIndex}:${intent.kind}:${intent.workspaceId}:${intent.assetId}`;
    case 'unrouted-deep-link':
      return `${intent.requestId}:${intent.targetIndex}:${intent.kind}:${intent.value}`;
  }
}

/** Returns a bounded preview hint; Rust remains the source and permission authority. */
export function contentKindHintForPath(path: string): string {
  const extension = path.toLowerCase().split('.').pop() ?? '';
  switch (extension) {
    case 'json':
      return 'application/json';
    case 'xml':
      return 'application/xml';
    case 'yaml':
    case 'yml':
      return 'application/yaml';
    case 'toml':
      return 'application/toml';
    case 'md':
      return 'text/markdown';
    case 'txt':
    case 'log':
    case 'csv':
      return 'text/plain';
    case 'png':
      return 'image/png';
    case 'jpg':
    case 'jpeg':
      return 'image/jpeg';
    case 'gif':
      return 'image/gif';
    case 'webp':
      return 'image/webp';
    case 'svg':
      return 'image/svg+xml';
    case 'pdf':
      return 'application/pdf';
    default:
      return 'application/octet-stream';
  }
}

function routeActivationTarget(
  requestId: string,
  target: CoreActivationTarget,
  targetIndex: number,
): WorkspaceRoutingIntent {
  if (target.kind === 'file-path') {
    return {
      kind: 'open-file',
      workspaceId: ASSET_MANAGER_WORKSPACE,
      requestId,
      targetIndex,
      path: target.value,
    };
  }

  const assetId = parseAssetDeepLink(target.value);
  if (assetId) {
    return {
      kind: 'open-asset',
      workspaceId: ASSET_MANAGER_WORKSPACE,
      requestId,
      targetIndex,
      assetId,
    };
  }

  return {
    kind: 'unrouted-deep-link',
    requestId,
    targetIndex,
    value: target.value,
    reason: 'unsupported-route',
  };
}

function parseAssetDeepLink(value: string): string | undefined {
  let url: URL;
  try {
    url = new URL(value);
  } catch {
    return undefined;
  }
  if (url.protocol !== 'velvet:' || url.hostname !== 'asset' || url.search || url.hash) {
    return undefined;
  }

  const segments = url.pathname.split('/').filter(Boolean);
  if (segments.length !== 1) {
    return undefined;
  }
  let assetId: string;
  try {
    assetId = decodeURIComponent(segments[0]);
  } catch {
    return undefined;
  }
  if (assetId.length === 0 || assetId.length > 512 || assetId.includes('\u0000')) {
    return undefined;
  }
  return assetId;
}
