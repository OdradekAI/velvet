import {
  PROTOCOL_VERSION,
  type CoreError,
  type CoreEvent,
  type CoreResponse,
  type CoreRequest,
  type CoreTransport,
  type ProtocolVersion,
} from './protocol.js';

export interface TauriInvoke {
  invoke<T>(command: string, args?: Readonly<Record<string, unknown>>): Promise<T>;
}

export interface TauriEvent<T> {
  readonly payload: T;
}

export interface TauriListen {
  listen<T>(
    event: string,
    listener: (event: TauriEvent<T>) => void,
  ): Promise<() => void>;
}

export type TauriBridge = TauriInvoke & TauriListen;

export interface TauriRequestEnvelope {
  readonly protocolVersion: ProtocolVersion;
  readonly requestId: number;
  readonly request: CoreRequest;
}

export interface TauriResponseEnvelope {
  readonly protocolVersion: ProtocolVersion;
  readonly requestId: number;
  readonly response: CoreResponse;
}

export interface TauriEventEnvelope {
  readonly protocolVersion: ProtocolVersion;
  readonly event: CoreEvent;
}

export interface TauriTransportOptions {
  readonly requestCommand?: string;
  readonly eventName?: string;
}

export class CoreTransportError extends Error {
  public readonly details: CoreError;

  public constructor(details: CoreError) {
    super(details.message);
    this.name = 'CoreTransportError';
    this.details = details;
  }
}

/**
 * Explicit mapping seam for the future native Tauri command and event
 * bindings. It intentionally knows nothing about React or the Tauri package.
 */
export class TauriCoreTransport implements CoreTransport {
  private readonly requestCommand: string;
  private readonly eventName: string;
  private nextRequestId = 1;
  private readonly listeners = new Set<(event: CoreEvent) => void>();
  private eventSubscription: Promise<(() => void)> | undefined;
  private disposed = false;

  public constructor(
    private readonly bridge: TauriBridge,
    options: TauriTransportOptions = {},
  ) {
    this.requestCommand = options.requestCommand ?? 'velvet_core_request';
    this.eventName = options.eventName ?? 'velvet_core_event';
  }

  public async request(request: CoreRequest): Promise<CoreResponse> {
    this.ensureUsable();
    const envelope: TauriRequestEnvelope = {
      protocolVersion: PROTOCOL_VERSION,
      requestId: this.nextRequestId++,
      request,
    };

    try {
      const response = await this.bridge.invoke<TauriResponseEnvelope>(
        this.requestCommand,
        { envelope },
      );
      this.validateResponse(envelope, response);
      return response.response;
    } catch (error) {
      if (error instanceof CoreTransportError) {
        throw error;
      }

      throw new CoreTransportError(normalizeCoreError(error));
    }
  }

  public subscribe(listener: (event: CoreEvent) => void): () => void {
    this.ensureUsable();
    this.listeners.add(listener);
    void this.ensureEventSubscription().catch(() => undefined);

    return () => {
      this.listeners.delete(listener);
    };
  }

  public async dispose(): Promise<void> {
    if (this.disposed) {
      return;
    }

    this.disposed = true;
    this.listeners.clear();

    const unsubscribe = this.eventSubscription;
    this.eventSubscription = undefined;
    if (unsubscribe) {
      (await unsubscribe)();
    }
  }

  private async ensureEventSubscription(): Promise<void> {
    if (this.eventSubscription || this.disposed || this.listeners.size === 0) {
      return;
    }

    this.eventSubscription = this.bridge.listen<TauriEventEnvelope>(
      this.eventName,
      ({ payload }) => this.handleEvent(payload),
    );

    try {
      const unsubscribe = await this.eventSubscription;
      if (this.disposed || this.listeners.size === 0) {
        unsubscribe();
        this.eventSubscription = undefined;
      }
    } catch (error) {
      this.eventSubscription = undefined;
      if (!this.disposed) {
        throw new CoreTransportError(normalizeCoreError(error));
      }
    }
  }

  private handleEvent(payload: TauriEventEnvelope): void {
    if (payload.protocolVersion !== PROTOCOL_VERSION) {
      return;
    }

    for (const listener of this.listeners) {
      try {
        listener(payload.event);
      } catch {
        // A failing projection listener must not interrupt other subscribers.
      }
    }
  }

  private validateResponse(
    request: TauriRequestEnvelope,
    response: TauriResponseEnvelope,
  ): void {
    if (response.protocolVersion !== PROTOCOL_VERSION) {
      throw new CoreTransportError({
        code: 'protocol-version-mismatch',
        message: `Expected protocol ${PROTOCOL_VERSION}, received ${response.protocolVersion}`,
        retryable: false,
      });
    }

    if (response.requestId !== request.requestId) {
      throw new CoreTransportError({
        code: 'request-id-mismatch',
        message: `Expected response for request ${request.requestId}, received ${response.requestId}`,
        retryable: true,
      });
    }
  }

  private ensureUsable(): void {
    if (this.disposed) {
      throw new CoreTransportError({
        code: 'transport-disposed',
        message: 'The Tauri core transport has been disposed.',
        retryable: false,
      });
    }
  }
}

function normalizeCoreError(error: unknown): CoreError {
  if (isCoreError(error)) {
    return error;
  }

  if (error instanceof Error) {
    return {
      code: 'transport-error',
      message: error.message,
      retryable: true,
    };
  }

  return {
    code: 'transport-error',
    message: 'The core transport request failed.',
    retryable: true,
  };
}

function isCoreError(value: unknown): value is CoreError {
  if (!value || typeof value !== 'object') {
    return false;
  }

  const candidate = value as Partial<CoreError>;
  return (
    typeof candidate.code === 'string' &&
    typeof candidate.message === 'string' &&
    typeof candidate.retryable === 'boolean'
  );
}
