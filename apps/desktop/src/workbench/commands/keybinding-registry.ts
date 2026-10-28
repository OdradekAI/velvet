import type { WorkbenchAppId } from '../../platform/protocol.js';
import {
  CommandRegistry,
  type CommandExecutionContext,
} from './command-registry.js';
import {
  ContextKeyService,
  type ContextKeySnapshot,
} from './context-key-service.js';

export type KeybindingScope = 'workbench' | 'workbench-app';

export interface KeybindingDescriptor {
  readonly id: string;
  readonly commandId: string;
  readonly keybinding: string;
  readonly scope: KeybindingScope;
  readonly namespace: string;
  readonly appId?: WorkbenchAppId;
  readonly when?: string;
  readonly order?: number;
}

export interface KeybindingResolutionContext {
  readonly activeAppId?: WorkbenchAppId;
  readonly keys?: ContextKeySnapshot;
}

/** Host-owned keybinding resolution; it has no browser or React dependency. */
export class KeybindingRegistry {
  private readonly keybindings = new Map<string, KeybindingDescriptor>();

  public constructor(
    private readonly commands: CommandRegistry,
    private readonly contextKeys: ContextKeyService,
  ) {}

  public register(descriptor: KeybindingDescriptor): void {
    validateKeybinding(descriptor);
    const key = keyFor(descriptor.namespace, descriptor.id);
    if (this.keybindings.has(key)) {
      throw new Error(`Duplicate keybinding: ${key}`);
    }

    this.keybindings.set(key, {
      ...descriptor,
      keybinding: normalizeKeybinding(descriptor.keybinding),
    });
  }

  public unregister(
    descriptor: Pick<KeybindingDescriptor, 'namespace' | 'id'>,
  ): boolean {
    return this.keybindings.delete(keyFor(descriptor.namespace, descriptor.id));
  }

  public resolve(
    keybinding: string,
    context: KeybindingResolutionContext = {},
  ): KeybindingDescriptor | undefined {
    const normalized = normalizeKeybinding(keybinding);
    const candidates = [...this.keybindings.values()]
      .filter((descriptor) => isVisible(descriptor, context.activeAppId))
      .filter((descriptor) => descriptor.keybinding === normalized)
      .filter((descriptor) => this.contextKeys.evaluate(descriptor.when, context.keys))
      .sort((left, right) => {
        const leftApp = left.scope === 'workbench-app' ? 1 : 0;
        const rightApp = right.scope === 'workbench-app' ? 1 : 0;
        return rightApp - leftApp || (left.order ?? 0) - (right.order ?? 0);
      });

    return candidates[0];
  }

  public list(context: KeybindingResolutionContext = {}): readonly KeybindingDescriptor[] {
    return [...this.keybindings.values()]
      .filter((descriptor) => isVisible(descriptor, context.activeAppId))
      .filter((descriptor) => this.contextKeys.evaluate(descriptor.when, context.keys))
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  }

  public async execute(
    keybinding: string,
    args: unknown,
    context: CommandExecutionContext,
  ): Promise<void> {
    const descriptor = this.resolve(keybinding, {
      activeAppId: context.activeAppId,
      keys: context.keys,
    });
    if (!descriptor) {
      throw new Error(`Keybinding is not visible: ${keybinding}`);
    }

    await this.commands.execute(descriptor.commandId, args, context);
  }
}

export function normalizeKeybinding(value: string): string {
  return value
    .trim()
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .map((chord) => {
      const parts = chord
        .split('+')
        .map((part) => normalizeKeyPart(part))
        .filter(Boolean);
      const modifiers = parts.filter((part) => MODIFIER_ORDER.includes(part));
      const keys = parts.filter((part) => !MODIFIER_ORDER.includes(part));
      modifiers.sort(
        (left, right) => MODIFIER_ORDER.indexOf(left) - MODIFIER_ORDER.indexOf(right),
      );
      return [...new Set([...modifiers, ...keys])].join('+');
    })
    .join(' ');
}

function validateKeybinding(descriptor: KeybindingDescriptor): void {
  if (!normalizeKeybinding(descriptor.keybinding)) {
    throw new Error(`Keybinding ${descriptor.id} must declare a key sequence`);
  }
  if (descriptor.scope === 'workbench' && descriptor.namespace !== 'workbench') {
    throw new Error(`Global keybinding ${descriptor.id} must use the workbench namespace`);
  }
  if (descriptor.scope === 'workbench-app') {
    if (!descriptor.appId) {
      throw new Error(`App keybinding ${descriptor.id} must declare an app id`);
    }
    if (descriptor.namespace !== `workbench.app.${descriptor.appId}`) {
      throw new Error(`App keybinding ${descriptor.id} has an invalid namespace`);
    }
  }
}

function normalizeKeyPart(value: string): string {
  switch (value) {
    case 'command':
    case 'cmd':
      return 'meta';
    case 'commandorcontrol':
    case 'cmdorcontrol':
      return 'ctrl';
    case 'control':
      return 'ctrl';
    case 'option':
      return 'alt';
    case 'return':
      return 'enter';
    case 'esc':
      return 'escape';
    default:
      return value;
  }
}

function keyFor(namespace: string, id: string): string {
  return `${namespace}:${id}`;
}

function isVisible(
  descriptor: KeybindingDescriptor,
  activeAppId: WorkbenchAppId | undefined,
): boolean {
  return descriptor.scope === 'workbench' || descriptor.appId === activeAppId;
}

const MODIFIER_ORDER = ['ctrl', 'alt', 'shift', 'meta'];
