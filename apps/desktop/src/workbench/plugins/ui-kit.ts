/**
 * Framework-neutral UI Kit contract shared by host adapters and isolated
 * plugin surfaces. This module contains no React, DOM or Tailwind types.
 */

export const UI_THEME_SCHEMA = 'velvet.ui.theme';
export const UI_THEME_SCHEMA_VERSION = 1;
export const SUPPORTED_UI_COMPONENT_CONTRACT_VERSION = 1;
export const UI_ENTRYPOINT_SCHEMES = [
  'http:',
  'https:',
  'tauri-webview:',
  'asset:',
  'data:',
  'blob:',
  'wasm:',
  'builtin:',
] as const;

export type UiThemeScheme = 'light' | 'dark';
export type UiDensity = 'compact' | 'default' | 'comfortable';
export type UiContrast = 'normal' | 'high';

export const UI_SEMANTIC_TOKENS = [
  'surface.canvas',
  'surface.panel',
  'surface.raised',
  'surface.overlay',
  'surface.active',
  'border.subtle',
  'border.strong',
  'text.primary',
  'text.secondary',
  'text.muted',
  'accent.default',
  'status.danger',
  'status.warning',
  'status.success',
  'status.info',
  'focus.default',
  'control.height',
  'control.radius',
] as const;

export type UiSemanticToken = (typeof UI_SEMANTIC_TOKENS)[number];

export interface UiThemeSnapshot {
  readonly schema: typeof UI_THEME_SCHEMA;
  readonly schemaVersion: number;
  readonly scheme: UiThemeScheme;
  readonly density: UiDensity;
  readonly contrast: UiContrast;
  readonly reducedMotion: boolean;
  readonly revision: number;
  readonly tokens: Readonly<Partial<Record<UiSemanticToken, string>>>;
}

export interface UiThemeSnapshotInput {
  readonly schema?: string;
  readonly schemaVersion?: number;
  readonly scheme: UiThemeScheme;
  readonly density?: UiDensity;
  readonly contrast?: UiContrast;
  readonly reducedMotion?: boolean;
  readonly revision: number;
  readonly tokens: Readonly<Record<string, string>>;
}

export const DEFAULT_UI_THEME: UiThemeSnapshot = {
  schema: UI_THEME_SCHEMA,
  schemaVersion: UI_THEME_SCHEMA_VERSION,
  scheme: 'dark',
  density: 'default',
  contrast: 'normal',
  reducedMotion: false,
  revision: 0,
  tokens: {},
};

export const HOST_ONLY_UI_SURFACES = [
  'activity-bar',
  'navigation-pane',
  'context-pane',
  'operations-panel',
  'preview-host',
  'global-overlay',
  'global-shortcuts',
] as const;

export type HostOnlyUiSurface = (typeof HOST_ONLY_UI_SURFACES)[number];

export const PLUGIN_UI_COMPONENTS = [
  'text',
  'icon',
  'button',
  'input',
  'select',
  'checkbox',
  'tabs',
  'menu',
  'dialog-body',
  'toast',
  'progress',
  'empty-state',
  'error-state',
  'loading-state',
  'list',
  'tree',
  'table',
  'grid',
] as const;

export type PluginUiComponent = (typeof PLUGIN_UI_COMPONENTS)[number];

export function isHostOnlyUiSurface(value: string): value is HostOnlyUiSurface {
  return (HOST_ONLY_UI_SURFACES as readonly string[]).includes(value);
}

export function isPluginUiComponent(value: string): value is PluginUiComponent {
  return (PLUGIN_UI_COMPONENTS as readonly string[]).includes(value);
}

export function negotiateUiComponentVersion(
  offered: readonly number[],
  supported: readonly number[] = [SUPPORTED_UI_COMPONENT_CONTRACT_VERSION],
): number | undefined {
  const compatible = offered.filter((version) => supported.includes(version));
  return compatible.length > 0 ? Math.max(...compatible) : undefined;
}

export function isAllowedUiEntrypoint(entrypoint: string): boolean {
  const scheme = entrypoint.match(/^([A-Za-z][A-Za-z0-9+.-]*:)/)?.[1]?.toLowerCase();
  if (scheme === undefined || !(UI_ENTRYPOINT_SCHEMES as readonly string[]).includes(scheme)) {
    return false;
  }
  if (scheme === 'http:' || scheme === 'https:' || scheme === 'tauri-webview:') {
    const hasAuthority = scheme === 'tauri-webview:'
      ? entrypoint.startsWith('tauri-webview://')
      : entrypoint.startsWith(scheme + '//');
    if (!hasAuthority) {
      return false;
    }
    const normalized = scheme === 'tauri-webview:'
      ? entrypoint.replace(/^tauri-webview:\/\//, 'https://')
      : entrypoint;
    try {
      return Boolean(new URL(normalized).hostname);
    } catch {
      return false;
    }
  }
  return entrypoint.length > scheme.length;
}

/**
 * Converts the legacy dark-only theme shape into the versioned semantic
 * snapshot. Unknown tokens and unsafe CSS values are deliberately discarded
 * before a snapshot crosses the isolated surface boundary.
 */
export function normalizeUiThemeSnapshot(input: UiThemeSnapshotInput): UiThemeSnapshot {
  if (input.schema && input.schema !== UI_THEME_SCHEMA) {
    throw new Error(`Unsupported UI theme schema: ${input.schema}`);
  }
  if (input.schemaVersion !== undefined && input.schemaVersion !== UI_THEME_SCHEMA_VERSION) {
    throw new Error(`Unsupported UI theme schema version: ${input.schemaVersion}`);
  }
  if (!Number.isInteger(input.revision) || input.revision < 0) {
    throw new Error('UI theme revision must be a non-negative integer');
  }
  if (input.scheme !== 'light' && input.scheme !== 'dark') {
    throw new Error(`Unsupported UI theme scheme: ${String(input.scheme)}`);
  }
  if (input.density !== undefined && !['compact', 'default', 'comfortable'].includes(input.density)) {
    throw new Error(`Unsupported UI theme density: ${String(input.density)}`);
  }
  if (input.contrast !== undefined && !['normal', 'high'].includes(input.contrast)) {
    throw new Error(`Unsupported UI theme contrast: ${String(input.contrast)}`);
  }

  const tokens: Partial<Record<UiSemanticToken, string>> = {};
  for (const token of UI_SEMANTIC_TOKENS) {
    const value = input.tokens[token];
    if (typeof value === 'string' && isSafeTokenValue(value)) {
      tokens[token] = value;
    }
  }

  return {
    schema: UI_THEME_SCHEMA,
    schemaVersion: UI_THEME_SCHEMA_VERSION,
    scheme: input.scheme,
    density: input.density ?? 'default',
    contrast: input.contrast ?? 'normal',
    reducedMotion: input.reducedMotion ?? false,
    revision: input.revision,
    tokens,
  };
}

function isSafeTokenValue(value: string): boolean {
  if (value.length === 0 || value.length > 256 || /[\u0000-\u001f]/.test(value)) {
    return false;
  }
  const normalized = value.toLowerCase();
  return !normalized.includes('url(')
    && !normalized.includes('expression(')
    && !normalized.includes('</style');
}
