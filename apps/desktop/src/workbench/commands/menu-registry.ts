import type { ContextAnchor } from '../contributions/contribution-registry.js';
import type { WorkbenchAppId } from '../../platform/protocol.js';
import {
  ContextKeyService,
  type ContextKeySnapshot,
} from './context-key-service.js';
import { CommandRegistry, type CommandScope } from './command-registry.js';

export type MenuLocation =
  | 'title-bar'
  | 'context-menu'
  | 'command-palette'
  | 'app-toolbar'
  | 'status-bar';

export interface MenuContribution {
  readonly id: string;
  readonly label: string;
  readonly labelKey?: string;
  readonly location: MenuLocation;
  readonly commandId: string;
  readonly scope: CommandScope;
  readonly namespace: string;
  readonly appId?: WorkbenchAppId;
  readonly group?: string;
  readonly order?: number;
  readonly when?: string;
  readonly anchor?: ContextAnchor;
}

export interface MenuContext {
  readonly activeAppId?: WorkbenchAppId;
  readonly anchor?: ContextAnchor;
  readonly keys?: ContextKeySnapshot;
}

export class MenuRegistry {
  private readonly menus = new Map<string, MenuContribution>();
  private readonly listeners = new Set<() => void>();

  public constructor(
    private readonly commands: CommandRegistry,
    private readonly contextKeys: ContextKeyService,
  ) {}

  public register(menu: MenuContribution): void {
    this.validate(menu);
    const key = keyFor(menu.namespace, menu.id);
    if (this.menus.has(key)) {
      throw new Error(`Duplicate menu contribution: ${key}`);
    }

    this.menus.set(key, menu);
    this.notify();
  }

  public unregister(menu: Pick<MenuContribution, 'namespace' | 'id'>): boolean {
    const removed = this.menus.delete(keyFor(menu.namespace, menu.id));
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public list(location: MenuLocation, context: MenuContext = {}): readonly MenuContribution[] {
    return [...this.menus.values()]
      .filter((menu) => menu.location === location)
      .filter((menu) => menu.scope === 'workbench' || menu.appId === context.activeAppId)
      .filter((menu) => !menu.anchor || menu.anchor === context.anchor)
      .filter((menu) => this.contextKeys.evaluate(menu.when, context.keys))
      .filter((menu) => {
        const command = this.commands.get(menu.namespace, menu.commandId);
        return command !== undefined;
      })
      .sort((left, right) => {
        const groupOrder = (left.group ?? '').localeCompare(right.group ?? '');
        return groupOrder || (left.order ?? 0) - (right.order ?? 0);
      });
  }

  private validate(menu: MenuContribution): void {
    if (menu.scope === 'workbench' && menu.namespace !== 'workbench') {
      throw new Error(`Global menu ${menu.id} must use the workbench namespace`);
    }
    if (menu.scope === 'workbench-app') {
      if (!menu.appId || menu.namespace !== `workbench.app.${menu.appId}`) {
        throw new Error(`App menu ${menu.id} has an invalid namespace or app id`);
      }
    }
    if (menu.location === 'app-toolbar' && menu.scope !== 'workbench-app') {
      throw new Error(`App toolbar menu ${menu.id} must be app-scoped`);
    }
    if (menu.location === 'context-menu' && !menu.anchor) {
      throw new Error(`Context menu ${menu.id} must declare an anchor`);
    }
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One menu projection must not prevent other host projections from updating.
      }
    }
  }

}

function keyFor(namespace: string, id: string): string {
  return `${namespace}:${id}`;
}
