import type { Part } from '../../platform/protocol.js';

export type FocusTarget =
  | { readonly kind: 'part'; readonly part: Part }
  | { readonly kind: 'view'; readonly viewId: string }
  | { readonly kind: 'overlay'; readonly instanceId: string };

export interface FocusSnapshot {
  readonly revision: number;
  readonly current?: FocusTarget;
  readonly previous?: FocusTarget;
}

export type FocusListener = (snapshot: FocusSnapshot) => void;

export function focusTargetId(target: FocusTarget): string {
  switch (target.kind) {
    case 'part':
      return `part:${target.part}`;
    case 'view':
      return `view:${target.viewId}`;
    case 'overlay':
      return `overlay:${target.instanceId}`;
  }
}

export function parseFocusTargetId(value: string): FocusTarget | undefined {
  const separator = value.indexOf(':');
  if (separator <= 0 || separator === value.length - 1) {
    return undefined;
  }

  const kind = value.slice(0, separator);
  const identifier = value.slice(separator + 1);
  if (kind === 'view') {
    return { kind, viewId: identifier };
  }
  if (kind === 'overlay') {
    return { kind, instanceId: identifier };
  }
  if (kind === 'part' && isPart(identifier)) {
    return { kind, part: identifier };
  }

  return undefined;
}

/** Framework-neutral focus ownership and restoration state for the Workbench. */
export class FocusService {
  private snapshotValue: FocusSnapshot = { revision: 0 };
  private readonly listeners = new Set<FocusListener>();

  public focus(target: FocusTarget): FocusSnapshot {
    this.snapshotValue = {
      revision: this.snapshotValue.revision + 1,
      current: target,
      previous: this.snapshotValue.current,
    };
    this.notify();
    return this.snapshotValue;
  }

  public restore(): FocusSnapshot {
    const current = this.snapshotValue.previous;
    this.snapshotValue = {
      revision: this.snapshotValue.revision + 1,
      current,
      previous: undefined,
    };
    this.notify();
    return this.snapshotValue;
  }

  public clear(): FocusSnapshot {
    this.snapshotValue = {
      revision: this.snapshotValue.revision + 1,
      previous: this.snapshotValue.current,
    };
    this.notify();
    return this.snapshotValue;
  }

  public snapshot(): FocusSnapshot {
    return this.snapshotValue;
  }

  public subscribe(listener: FocusListener): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener(this.snapshotValue);
      } catch {
        // One projection must not prevent the host from notifying others.
      }
    }
  }
}

function isPart(value: string): value is Part {
  return [
    'title-bar',
    'activity-bar',
    'primary-sidebar',
    'workspace-area',
    'auxiliary-sidebar',
    'operations-panel',
    'status-bar',
    'overlay-layer',
  ].includes(value);
}
