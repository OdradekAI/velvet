import type { Zone } from '../protocol.js';

export interface ViewDescriptor {
  id: string;
  containerId: string;
  title: string;
  zone: Zone;
  defaultVisible?: boolean;
}

export class ViewRegistry {
  private readonly views = new Map<string, ViewDescriptor>();

  public register(view: ViewDescriptor): void {
    if (this.views.has(view.id)) {
      throw new Error(`Duplicate view id: ${view.id}`);
    }

    this.views.set(view.id, view);
  }

  public get(id: string): ViewDescriptor | undefined {
    return this.views.get(id);
  }

  public list(zone?: Zone): ViewDescriptor[] {
    return [...this.views.values()].filter((view) => zone === undefined || view.zone === zone);
  }
}
