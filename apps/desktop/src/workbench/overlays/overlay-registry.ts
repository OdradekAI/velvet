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
  readonly namespace: string;
  readonly focusToken?: string;
}

export class OverlayRegistry {
  private readonly descriptors = new Map<string, OverlayDescriptor>();
  private readonly instances: OverlayInstance[] = [];
  private readonly listeners = new Set<() => void>();
  private sequence = 0;

  public constructor(private readonly contextKeys: ContextKeyService) {}

  public register(descriptor: OverlayDescriptor): void {
    this.validate(descriptor);
    const key = keyFor(descriptor.namespace, descriptor.id);
    if (this.descriptors.has(key)) {
      throw new Error(`Duplicate overlay descriptor: ${key}`);
    }

    this.descriptors.set(key, descriptor);
    this.notify();
  }

  public get(
    namespace: string,
    descriptorId: string,
  ): OverlayDescriptor | undefined {
    return this.descriptors.get(keyFor(namespace, descriptorId));
  }

  public unregister(descriptor: Pick<OverlayDescriptor, 'namespace' | 'id'>): boolean {
    const key = keyFor(descriptor.namespace, descriptor.id);
    const removed = this.descriptors.delete(key);
    for (let index = this.instances.length - 1; index >= 0; index -= 1) {
      const instance = this.instances[index];
      if (instance.namespace === descriptor.namespace && instance.descriptorId === descriptor.id) {
        this.instances.splice(index, 1);
      }
    }
    if (removed) {
      this.notify();
    }
    return removed;
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
      namespace: descriptor.namespace,
      focusToken: invocation.focusToken,
    };
    this.instances.push(instance);
    this.notify();
    return instance;
  }

  public close(instanceId: string): OverlayInstance | undefined {
    const index = this.instances.findIndex((instance) => instance.instanceId === instanceId);
    if (index < 0) {
      return undefined;
    }

    const instance = this.instances.splice(index, 1)[0];
    this.notify();
    return instance;
  }

  public closeTop(): OverlayInstance | undefined {
    const instance = this.instances.pop();
    if (instance) {
      this.notify();
    }
    return instance;
  }

  public active(): readonly OverlayInstance[] {
    return [...this.instances];
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
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

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One overlay projection must not prevent other host projections from updating.
      }
    }
  }
}

function keyFor(namespace: string, id: string): string {
  return `${namespace}:${id}`;
}
