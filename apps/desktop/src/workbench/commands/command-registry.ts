import type {
  CorePreviewActionContext,
  WorkbenchAppId,
} from '../../platform/protocol.js';
import {
  ContextKeyService,
  type ContextKeySnapshot,
} from './context-key-service.js';

export type CommandScope = 'workbench' | 'workbench-app';

export interface CommandExecutionContext {
  readonly activeAppId?: WorkbenchAppId;
  readonly keys: ContextKeySnapshot;
  readonly preview?: CorePreviewActionContext;
}

export type CommandHandler = (
  args: unknown,
  context: CommandExecutionContext,
) => unknown | Promise<unknown>;

export interface CommandDescriptor {
  readonly id: string;
  readonly title: string;
  readonly titleKey?: string;
  readonly scope: CommandScope;
  readonly namespace: string;
  readonly appId?: WorkbenchAppId;
  readonly when?: string;
  readonly keybinding?: string;
  readonly order?: number;
  readonly execute?: CommandHandler;
}

export class CommandRegistry {
  private readonly commands = new Map<string, CommandDescriptor>();

  public constructor(private readonly contextKeys: ContextKeyService) {}

  public register(command: CommandDescriptor): void {
    this.validate(command);
    const key = keyFor(command.namespace, command.id);
    if (this.commands.has(key)) {
      throw new Error(`Duplicate command: ${key}`);
    }

    this.commands.set(key, command);
  }

  public unregister(command: Pick<CommandDescriptor, 'namespace' | 'id'>): boolean {
    return this.commands.delete(keyFor(command.namespace, command.id));
  }

  public get(namespace: string, id: string): CommandDescriptor | undefined {
    return this.commands.get(keyFor(namespace, id));
  }

  public resolve(id: string, activeAppId?: WorkbenchAppId): CommandDescriptor | undefined {
    if (activeAppId) {
      const appCommand = this.get(`workbench.app.${activeAppId}`, id);
      if (appCommand) {
        return appCommand;
      }
    }

    return this.get('workbench', id);
  }

  public list(activeAppId?: WorkbenchAppId): readonly CommandDescriptor[] {
    return [...this.commands.values()]
      .filter((command) => isVisible(command, activeAppId))
      .filter((command) => this.contextKeys.evaluate(command.when))
      .sort((left, right) => (left.order ?? 0) - (right.order ?? 0));
  }

  public async execute(
    id: string,
    args: unknown,
    context: CommandExecutionContext,
  ): Promise<unknown> {
    const command = this.resolve(id, context.activeAppId);
    if (!command) {
      throw new Error(`Command is not visible: ${id}`);
    }
    if (!this.contextKeys.evaluate(command.when, context.keys)) {
      throw new Error(`Command is disabled by its context: ${id}`);
    }
    if (!command.execute) {
      throw new Error(`Command has no host handler: ${id}`);
    }

    return command.execute(args, context);
  }

  private validate(command: CommandDescriptor): void {
    if (command.scope === 'workbench' && command.namespace !== 'workbench') {
      throw new Error(`Global command ${command.id} must use the workbench namespace`);
    }
    if (command.scope === 'workbench-app') {
      if (!command.appId) {
        throw new Error(`App command ${command.id} must declare an app id`);
      }
      if (command.namespace !== `workbench.app.${command.appId}`) {
        throw new Error(`App command ${command.id} has an invalid namespace`);
      }
    }
  }
}

function keyFor(namespace: string, id: string): string {
  return `${namespace}:${id}`;
}

function isVisible(command: CommandDescriptor, activeAppId?: WorkbenchAppId): boolean {
  return command.scope === 'workbench' || command.appId === activeAppId;
}
