import type { ReactNode } from 'react';
import type { ViewDescriptor } from '../../workbench/views/view-registry.js';

export type ViewRenderer = (view: ViewDescriptor) => ReactNode;

/** React-only adapter registry; ViewDescriptor remains framework-independent. */
export class ViewRendererRegistry {
  private readonly renderers = new Map<string, ViewRenderer>();
  private readonly listeners = new Set<() => void>();

  public register(viewId: string, renderer: ViewRenderer): void {
    if (this.renderers.has(viewId)) {
      throw new Error(`Duplicate view renderer: ${viewId}`);
    }

    this.renderers.set(viewId, renderer);
    this.notify();
  }

  public unregister(viewId: string): boolean {
    const removed = this.renderers.delete(viewId);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public get(viewId: string): ViewRenderer | undefined {
    return this.renderers.get(viewId);
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One renderer projection must not prevent other view projections from updating.
      }
    }
  }
}
