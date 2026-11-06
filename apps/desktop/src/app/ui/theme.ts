import {
  DEFAULT_UI_THEME,
  LIGHT_UI_THEME,
  UI_SEMANTIC_TOKENS,
  normalizeUiThemeSnapshot,
  type UiSemanticToken,
  type UiThemeSnapshot,
} from '../../workbench/plugins/ui-kit.js';

/** Public semantic token to host CSS variable mapping. */
export const UI_TOKEN_CSS_VARIABLES: Readonly<Record<UiSemanticToken, string>> = {
  'surface.canvas': '--velvet-surface-app',
  'surface.panel': '--velvet-surface-panel',
  'surface.raised': '--velvet-surface-raised',
  'surface.overlay': '--velvet-surface-overlay',
  'surface.active': '--velvet-surface-active',
  'border.subtle': '--velvet-border-subtle',
  'border.strong': '--velvet-border-strong',
  'text.primary': '--velvet-text-primary',
  'text.secondary': '--velvet-text-secondary',
  'text.muted': '--velvet-text-muted',
  'accent.default': '--velvet-accent',
  'status.danger': '--velvet-status-danger',
  'status.warning': '--velvet-status-warning',
  'status.success': '--velvet-status-success',
  'status.info': '--velvet-status-info',
  'focus.default': '--velvet-focus',
  'control.height': '--velvet-control-height',
  'control.radius': '--velvet-radius-sm',
};

/** Applies a host-owned theme snapshot without exposing CSSOM to a plugin. */
export function applyUiThemeSnapshot(
  snapshot: UiThemeSnapshot,
  root?: HTMLElement,
): void {
  const target = root ?? (typeof document === 'undefined' ? undefined : document.documentElement);
  if (!target) {
    return;
  }

  target.dataset.velvetScheme = snapshot.scheme;
  target.dataset.velvetDensity = snapshot.density;
  target.dataset.velvetContrast = snapshot.contrast;
  target.dataset.velvetReducedMotion = snapshot.reducedMotion ? 'true' : 'false';
  target.dataset.velvetThemeRevision = String(snapshot.revision);

  for (const token of UI_SEMANTIC_TOKENS) {
    const variable = UI_TOKEN_CSS_VARIABLES[token];
    const value = snapshot.tokens[token];
    if (value === undefined) {
      target.style.removeProperty(variable);
    } else {
      target.style.setProperty(variable, value);
    }
  }
}

export function uiTokenVariable(token: UiSemanticToken): string {
  return `var(${UI_TOKEN_CSS_VARIABLES[token]})`;
}

/** Renderer-owned theme state. Rust/profile persistence can project into this service later. */
export class UiThemeService {
  private snapshotValue: UiThemeSnapshot;
  private readonly listeners = new Set<(snapshot: UiThemeSnapshot) => void>();

  public constructor(initial: UiThemeSnapshot = DEFAULT_UI_THEME) {
    this.snapshotValue = initial;
  }

  public get snapshot(): UiThemeSnapshot {
    return this.snapshotValue;
  }

  public set(snapshot: UiThemeSnapshot): void {
    this.snapshotValue = normalizeUiThemeSnapshot(snapshot);
    applyUiThemeSnapshot(this.snapshotValue);
    for (const listener of this.listeners) {
      listener(this.snapshotValue);
    }
  }

  public toggleScheme(): UiThemeSnapshot {
    const source = this.snapshotValue.scheme === 'dark' ? LIGHT_UI_THEME : DEFAULT_UI_THEME;
    const next: UiThemeSnapshot = { ...source, revision: this.snapshotValue.revision + 1 };
    this.set(next);
    return next;
  }

  public subscribe(listener: (snapshot: UiThemeSnapshot) => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }
}
