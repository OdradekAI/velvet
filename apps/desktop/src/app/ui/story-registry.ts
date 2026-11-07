import type { ReactNode } from 'react';
import type { UiKitComponentId } from './component-meta.js';
import type { UiKitAsyncLifecycle } from './internal/component-contract.js';
import { badgeStories } from './stories/primitives/badge.stories.js';
import { buttonStories } from './stories/primitives/button.stories.js';
import { checkboxStories } from './stories/primitives/checkbox.stories.js';
import { iconButtonStories } from './stories/primitives/icon-button.stories.js';
import { iconStories } from './stories/primitives/icon.stories.js';
import { inlineStories } from './stories/primitives/inline.stories.js';
import { inputStories } from './stories/primitives/input.stories.js';
import { progressStories } from './stories/primitives/progress.stories.js';
import { selectStories } from './stories/primitives/select.stories.js';
import { separatorStories } from './stories/primitives/separator.stories.js';
import { stackStories } from './stories/primitives/stack.stories.js';
import { stateStories } from './stories/primitives/state.stories.js';
import { tabsStories } from './stories/primitives/tabs.stories.js';
import { textStories } from './stories/primitives/text.stories.js';
import { velvetUiStories } from './stories/velvet-ui.stories.js';

export interface UiKitStoryHostCall {
  readonly method: string;
  readonly payload?: unknown;
}

export interface UiKitStoryHostAdapter {
  readonly calls: UiKitStoryHostCall[];
  record(method: string, payload?: unknown): void;
  createAsyncOperation<T>(method: string): UiKitStoryAsyncOperation<T>;
}

export type UiKitStoryAsyncState = 'requested' | 'pending' | 'committed' | 'cancelled' | 'timed-out' | 'failed' | 'retrying';

export interface UiKitStoryAsyncOperation<T> {
  readonly method: string;
  readonly promise: Promise<T>;
  readonly state: UiKitStoryAsyncState;
  readonly history: readonly UiKitStoryAsyncState[];
  start(): void;
  resolve(value: T): void;
  cancel(reason?: string): void;
  timeout(): void;
  fail(error?: Error): void;
  retry(): void;
}

export interface UiKitStoryRuntime {
  readonly root: HTMLElement;
  readonly act: (work: () => void | Promise<void>) => Promise<void>;
  readonly host: UiKitStoryHostAdapter;
}

export interface UiKitStoryCheckpoint {
  readonly name: string;
  readonly afterAction?: number;
  readonly assert: (runtime: UiKitStoryRuntime) => void | Promise<void>;
}

export interface UiKitStoryScenario {
  readonly covers?: readonly string[];
  readonly asyncLifecycle?: UiKitAsyncLifecycle;
  readonly setup?: (runtime: UiKitStoryRuntime) => void | Promise<void>;
  readonly actions: readonly ((runtime: UiKitStoryRuntime) => void | Promise<void>)[];
  readonly checkpoints: readonly UiKitStoryCheckpoint[];
  readonly cleanup?: (runtime: UiKitStoryRuntime) => void | Promise<void>;
}

const primitiveStories: readonly UiKitStory[] = [
  ...textStories,
  ...iconStories,
  ...badgeStories,
  ...stackStories,
  ...inlineStories,
  ...buttonStories,
  ...iconButtonStories,
  ...inputStories,
  ...selectStories,
  ...checkboxStories,
  ...separatorStories,
  ...tabsStories,
  ...progressStories,
  ...stateStories,
];

export interface UiKitStory {
  readonly id: string;
  readonly componentId: UiKitComponentId;
  readonly name: string;
  readonly description: string;
  readonly render: () => ReactNode;
  readonly scenario?: UiKitStoryScenario;
}

export function createUiKitStoryHostAdapter(): UiKitStoryHostAdapter {
  const calls: UiKitStoryHostCall[] = [];
  return {
    calls,
    record(method, payload) {
      calls.push(payload === undefined ? { method } : { method, payload });
    },
    createAsyncOperation<T>(method: string): UiKitStoryAsyncOperation<T> {
      let state: UiKitStoryAsyncState = 'requested';
      const history: UiKitStoryAsyncState[] = [state];
      let resolvePromise: (value: T) => void = () => undefined;
      let rejectPromise: (error: Error) => void = () => undefined;
      let deferred = createDeferred<T>();
      deferred.promise.catch(() => undefined);
      resolvePromise = deferred.resolve;
      rejectPromise = deferred.reject;
      calls.push({ method: `${method}.requested` });

      const transition = (next: UiKitStoryAsyncState) => {
        state = next;
        history.push(next);
        calls.push({ method: `${method}.${next}` });
      };
      const requirePending = () => {
        if (state !== 'pending') {
          throw new Error(`Async story operation ${method} is ${state}; expected pending`);
        }
      };

      return {
        method,
        get promise() {
          return deferred.promise;
        },
        get state() {
          return state;
        },
        get history() {
          return [...history];
        },
        start() {
          if (state !== 'requested') {
            throw new Error(`Async story operation ${method} cannot start from ${state}`);
          }
          transition('pending');
        },
        resolve(value) {
          requirePending();
          transition('committed');
          resolvePromise(value);
        },
        cancel(reason = 'cancelled') {
          requirePending();
          transition('cancelled');
          rejectPromise(new Error(reason));
        },
        timeout() {
          requirePending();
          transition('timed-out');
          rejectPromise(new Error('timed out'));
        },
        fail(error = new Error('failed')) {
          requirePending();
          transition('failed');
          rejectPromise(error);
        },
        retry() {
          if (state !== 'failed' && state !== 'cancelled' && state !== 'timed-out') {
            throw new Error(`Async story operation ${method} cannot retry from ${state}`);
          }
          transition('retrying');
          deferred = createDeferred<T>();
          deferred.promise.catch(() => undefined);
          resolvePromise = deferred.resolve;
          rejectPromise = deferred.reject;
          transition('pending');
        },
      } satisfies UiKitStoryAsyncOperation<T>;
    },
  };
}

function createDeferred<T>(): {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (error: Error) => void;
} {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((promiseResolve, promiseReject) => {
    resolve = promiseResolve;
    reject = promiseReject;
  });
  return { promise, resolve, reject };
}

export async function runUiKitStoryScenario(
  scenario: UiKitStoryScenario,
  runtime: UiKitStoryRuntime,
): Promise<void> {
  await scenario.setup?.(runtime);
  for (let actionIndex = 0; actionIndex < scenario.actions.length; actionIndex += 1) {
    await scenario.actions[actionIndex]?.(runtime);
    for (const checkpoint of scenario.checkpoints) {
      if (checkpoint.afterAction === actionIndex) {
        await checkpoint.assert(runtime);
      }
    }
  }
  for (const checkpoint of scenario.checkpoints) {
    if (checkpoint.afterAction === undefined) {
      await checkpoint.assert(runtime);
    }
  }
  await scenario.cleanup?.(runtime);
}

export class UiKitStoryRegistry {
  private readonly storiesById: ReadonlyMap<string, UiKitStory>;

  public constructor(stories: readonly UiKitStory[]) {
    const entries = stories.map((story) => [story.id, story] as const);
    if (new Set(entries.map(([id]) => id)).size !== entries.length) {
      throw new Error('Velvet UI Kit story ids must be unique');
    }
    this.storiesById = new Map(entries);
  }

  public list(): readonly UiKitStory[] {
    return [...this.storiesById.values()];
  }

  public forComponent(componentId: UiKitComponentId): readonly UiKitStory[] {
    return this.list().filter((story) => story.componentId === componentId);
  }

  public get(id: string): UiKitStory | undefined {
    return this.storiesById.get(id);
  }
}

export const UI_KIT_STORY_REGISTRY = new UiKitStoryRegistry([...primitiveStories, ...velvetUiStories]);
