import {
  PROTOCOL_MAJOR,
  PROTOCOL_MINOR,
  PROTOCOL_VERSION,
  PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
  PLUGIN_RESOURCE_SCHEMA_VERSION,
  PROFILE_SCHEMA_VERSION,
  RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
  RECOVERY_SCHEMA_VERSION,
  CRASH_CAPTURE_SCHEMA_VERSION,
  TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1,
  TRANSPORT_FEATURE_SESSION_V1,
  type CoreError,
  type CoreAuthoritativeSnapshot,
  type CoreRequestOutcome,
  type CoreEvent,
  type CoreResponse,
  type CoreRequest,
  type CoreTransport,
  type ProtocolHandshakeRequest,
  type ProtocolHandshakeResponse,
  type SessionAwareCoreTransport,
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
  readonly sessionId?: string;
  readonly request: CoreRequest;
}

export interface TauriResponseEnvelope {
  readonly protocolVersion: ProtocolVersion;
  readonly requestId: number;
  readonly response: CoreResponse;
}

export interface TauriEventEnvelope {
  readonly protocolVersion: ProtocolVersion;
  readonly sequence: number;
  readonly event: CoreEvent;
}

export interface TauriTransportOptions {
  readonly requestCommand?: string;
  readonly handshakeCommand?: string;
  readonly eventName?: string;
  readonly clientFeatures?: readonly string[];
  readonly schemaVersions?: Readonly<Record<string, number>>;
  readonly onSequenceGap?: (expected: number, received: number) => void;
  readonly onResync?: (snapshot: CoreAuthoritativeSnapshot) => void;
}

export const DEFAULT_TRANSPORT_SCHEMA_VERSIONS: Readonly<Record<string, number>> = {
  profile: PROFILE_SCHEMA_VERSION,
  recovery: RECOVERY_SCHEMA_VERSION,
  'recovery-diagnostic': RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
  'plugin-resource': PLUGIN_RESOURCE_SCHEMA_VERSION,
  'plugin-diagnostic': PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
  'crash-capture': CRASH_CAPTURE_SCHEMA_VERSION,
};

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
export class TauriCoreTransport implements SessionAwareCoreTransport {
  private readonly requestCommand: string;
  private readonly handshakeCommand: string;
  private readonly eventName: string;
  private readonly clientFeatures: readonly string[];
  private readonly schemaVersions: Readonly<Record<string, number>>;
  private readonly clientId: string;
  private readonly onSequenceGap?: (expected: number, received: number) => void;
  private readonly onResync?: (snapshot: CoreAuthoritativeSnapshot) => void;
  private nextRequestId = 1;
  private sessionId: string | undefined;
  private handshakeResponse: ProtocolHandshakeResponse | undefined;
  private handshakePromise: Promise<ProtocolHandshakeResponse> | undefined;
  private resyncPromise: Promise<CoreAuthoritativeSnapshot> | undefined;
  private lastEventSequence: number | undefined;
  private readonly listeners = new Set<(event: CoreEvent) => void>();
  private eventSubscription: Promise<(() => void)> | undefined;
  private disposed = false;

  public constructor(
    private readonly bridge: TauriBridge,
    options: TauriTransportOptions = {},
  ) {
    this.requestCommand = options.requestCommand ?? 'velvet_core_request';
    this.handshakeCommand = options.handshakeCommand ?? 'velvet_core_handshake';
    this.eventName = options.eventName ?? 'velvet_core_event';
    this.clientFeatures = options.clientFeatures ?? [
      TRANSPORT_FEATURE_SESSION_V1,
      TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1,
    ];
    this.schemaVersions = options.schemaVersions ?? DEFAULT_TRANSPORT_SCHEMA_VERSIONS;
    this.clientId = createClientId();
    this.onSequenceGap = options.onSequenceGap;
    this.onResync = options.onResync;
  }

  public async request(request: CoreRequest): Promise<CoreResponse> {
    this.ensureUsable();
    const session = await this.connect();
    const envelope: TauriRequestEnvelope = {
      protocolVersion: PROTOCOL_VERSION,
      requestId: this.nextRequestId++,
      sessionId: session.sessionId,
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
        if (error.details.code === 'session-expired') {
          this.sessionId = undefined;
          this.handshakeResponse = undefined;
        }
        throw error;
      }

      throw new CoreTransportError(normalizeCoreError(error));
    }
  }

  public async connect(): Promise<ProtocolHandshakeResponse> {
    this.ensureUsable();
    if (this.handshakeResponse && this.sessionId) {
      return this.handshakeResponse;
    }
    if (this.handshakePromise) {
      return this.handshakePromise;
    }

    const request: ProtocolHandshakeRequest = {
      clientId: this.clientId,
      protocolMajor: PROTOCOL_MAJOR,
      protocolMinor: PROTOCOL_MINOR,
      features: this.clientFeatures,
      schemaVersions: this.schemaVersions,
    };
    this.handshakePromise = this.bridge
      .invoke<ProtocolHandshakeResponse>(this.handshakeCommand, { request })
      .then((response) => {
        this.validateHandshake(response, request);
        this.sessionId = response.sessionId;
        this.handshakeResponse = response;
        return response;
      })
      .catch((error) => {
        if (error instanceof CoreTransportError) {
          throw error;
        }
        throw new CoreTransportError(normalizeCoreError(error));
      })
      .finally(() => {
        this.handshakePromise = undefined;
      });

    return this.handshakePromise;
  }

  public async reconnect(): Promise<ProtocolHandshakeResponse> {
    this.sessionId = undefined;
    this.handshakeResponse = undefined;
    const response = await this.connect();
    // Event sequence numbers are scoped to the native core process. A fresh
    // handshake therefore starts a new observation window after a restart.
    this.lastEventSequence = undefined;
    return response;
  }

  public async resync(): Promise<CoreAuthoritativeSnapshot> {
    this.ensureUsable();
    const response = await this.request({ protocolVersion: PROTOCOL_VERSION, kind: 'query-snapshot' });
    if (response.kind !== 'snapshot') {
      throw new CoreTransportError({
        code: 'invalid-resync-response',
        message: 'The core returned a non-snapshot response for resync.',
        retryable: true,
      });
    }
    return response.snapshot;
  }

  public async queryRequestOutcome(requestId: number): Promise<CoreRequestOutcome> {
    this.ensureUsable();
    const response = await this.request({
      protocolVersion: PROTOCOL_VERSION,
      kind: 'query-request-outcome',
      requestId,
    });
    if (response.kind !== 'request-outcome') {
      throw new CoreTransportError({
        code: 'invalid-request-outcome-response',
        message: 'The core returned a non-outcome response for an outcome query.',
        retryable: true,
      });
    }
    return response.outcome;
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
    if (this.resyncPromise) {
      return;
    }

    if (payload.sequence <= 0 || (this.lastEventSequence !== undefined && payload.sequence <= this.lastEventSequence)) {
      return;
    }
    const expected = this.lastEventSequence === undefined ? payload.sequence : this.lastEventSequence + 1;
    this.lastEventSequence = payload.sequence;
    if (payload.sequence !== expected) {
      try {
        this.onSequenceGap?.(expected, payload.sequence);
      } catch {
        // A diagnostic callback must not prevent the transport from resyncing.
      }
      if (!this.resyncPromise) {
        this.resyncPromise = this.reconnect()
          .then(() => this.resync())
          .then((snapshot) => {
            this.onResync?.(snapshot);
            return snapshot;
          })
          .finally(() => {
            this.resyncPromise = undefined;
          });
        void this.resyncPromise.catch(() => undefined);
      }
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

  private validateHandshake(
    response: ProtocolHandshakeResponse,
    request: ProtocolHandshakeRequest,
  ): void {
    if (response.protocolMajor !== PROTOCOL_MAJOR) {
      throw new CoreTransportError({
        code: 'protocol-major-mismatch',
        message: `Expected protocol major ${PROTOCOL_MAJOR}, received ${response.protocolMajor}`,
        retryable: false,
      });
    }
    if (response.protocolMinor > request.protocolMinor || response.sessionId.length === 0) {
      throw new CoreTransportError({
        code: 'invalid-handshake',
        message: 'The core returned an invalid transport handshake.',
        retryable: false,
      });
    }
    if (response.selectedFeatures.some((feature) => !request.features.includes(feature))) {
      throw new CoreTransportError({
        code: 'invalid-handshake',
        message: 'The core selected a feature that the client did not advertise.',
        retryable: false,
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

function createClientId(): string {
  const uuid = globalThis.crypto?.randomUUID?.();
  return uuid ? `desktop-${uuid}` : `desktop-${Math.random().toString(36).slice(2)}`;
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
