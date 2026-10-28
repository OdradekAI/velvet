import type { WorkbenchAppId } from '../../platform/protocol.js';
import {
  ContextKeyService,
  type ContextKeySnapshot,
} from '../commands/context-key-service.js';

export type OverlayKind =
  | 'dialog'
  | 'popover'
  | 'notification'
  | 'context-menu'
  | 'command-search';

export type OverlayScope = 'workbench' | 'workbench-app';

export interface OverlayDescriptor {
  readonly id: string;
  readonly title: string;
  readonly kind: OverlayKind;
  readonly scope: OverlayScope;
  readonly namespace: string;
  readonly appId?: WorkbenchAppId;
  readonly modal: boolean;
  readonly dismissOnEscape: boolean;
  readonly restoreFocus: boolean;
  readonly when?: string;
  readonly timeoutMs?: number;
}

export interface OverlayInvocation {
  readonly activeAppId?: WorkbenchAppId;
  readonly keys?: ContextKeySnapshot;
  readonly focusToken?: string;
}

export interface OverlayInstance {
  readonly instanceId: string;
  readonly descriptorId: string;
  readonly focusToken?: string;
}

export class OverlayRegistry {
  private readonly descriptors = new Map<string, OverlayDescriptor>();
  private readonly instances: OverlayInstance[] = [];
  private sequence = 0;

  public constructor(private readonly contextKeys: ContextKeyService) {}

  public register(descriptor: OverlayDescriptor): void {
    this.validate(descriptor);
    const key = keyFor(descriptor.namespace, descriptor.id);
    if (this.descriptors.has(key)) {
      throw new Error(`Duplicate overlay descriptor: ${key}`);
    }

    this.descriptors.set(key, descriptor);
  }

  public open(descriptorId: string, invocation: OverlayInvocation = {}): OverlayInstance {
    const descriptor = this.resolve(descriptorId, invocation.activeAppId);
    if (!descriptor) {
      throw new Error(`Unknown overlay descriptor: ${descriptorId}`);
    }
    if (descriptor.scope === 'workbench-app' && descriptor.appId !== invocation.activeAppId) {
      throw new Error(`Overlay is not visible in the active app: ${descriptorId}`);
    }
    if (!this.contextKeys.evaluate(descriptor.when, invocation.keys)) {
      throw new Error(`Overlay is disabled by its context: ${descriptorId}`);
    }

    const instance: OverlayInstance = {
      instanceId: `${descriptorId}:${++this.sequence}`,
      descriptorId,
      focusToken: invocation.focusToken,
    };
    this.instances.push(instance);
    return instance;
  }

  public close(instanceId: string): OverlayInstance | undefined {
    const index = this.instances.findIndex((instance) => instance.instanceId === instanceId);
    if (index < 0) {
      return undefined;
    }

    return this.instances.splice(index, 1)[0];
  }

  public closeTop(): OverlayInstance | undefined {
    return this.instances.pop();
  }

  public active(): readonly OverlayInstance[] {
    return [...this.instances];
  }

  private validate(descriptor: OverlayDescriptor): void {
    if (descriptor.scope === 'workbench' && descriptor.namespace !== 'workbench') {
      throw new Error(`Global overlay ${descriptor.id} must use the workbench namespace`);
    }
    if (descriptor.scope === 'workbench-app') {
      if (!descriptor.appId || descriptor.namespace !== `workbench.app.${descriptor.appId}`) {
        throw new Error(`App overlay ${descriptor.id} has an invalid namespace or app id`);
      }
    }
  }

  private resolve(
    descriptorId: string,
    activeAppId: WorkbenchAppId | undefined,
  ): OverlayDescriptor | undefined {
    if (activeAppId) {
      const appDescriptor = this.descriptors.get(
        keyFor(`workbench.app.${activeAppId}`, descriptorId),
      );
      if (appDescriptor) {
        return appDescriptor;
      }
    }

    return this.descriptors.get(keyFor('workbench', descriptorId));
  }
}

function keyFor(namespace: string, id: string): string {
  return `${namespace}:${id}`;
}
