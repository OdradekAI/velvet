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
  ACTIVATION_SCHEMA_VERSION,
  RELEASE_MANIFEST_SCHEMA_VERSION,
  RELEASE_TRANSACTION_SCHEMA_VERSION,
  JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION,
  OPERATION_SCHEMA_VERSION,
  PORTABLE_BUNDLE_SCHEMA_VERSION,
  IMPORT_PLAN_SCHEMA_VERSION,
  IMPORT_RECEIPT_SCHEMA_VERSION,
  TRANSPORT_FEATURE_SCHEMA_NEGOTIATION_V1,
  TRANSPORT_FEATURE_SESSION_V1,
  type CoreError,
  type CoreActivationRequest,
  type CoreAuthoritativeSnapshot,
  type CoreRequestOutcome,
  type CoreEvent,
  type CoreResponse,
  type CoreRequest,
  type CoreTransport,
  type CoreTransportStatus,
  type ProtocolHandshakeRequest,
  type ProtocolHandshakeResponse,
  type SessionAwareCoreTransport,
  type ActivationAwareCoreTransport,
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
  readonly activationEventName?: string;
  readonly activationPendingCommand?: string;
  readonly clientFeatures?: readonly string[];
  readonly schemaVersions?: Readonly<Record<string, number>>;
  readonly onSequenceGap?: (expected: number, received: number) => void;
  readonly onResync?: (snapshot: CoreAuthoritativeSnapshot) => void | Promise<void>;
}

export const DEFAULT_TRANSPORT_SCHEMA_VERSIONS: Readonly<Record<string, number>> = {
  profile: PROFILE_SCHEMA_VERSION,
  recovery: RECOVERY_SCHEMA_VERSION,
  'recovery-diagnostic': RECOVERY_DIAGNOSTIC_SCHEMA_VERSION,
  'plugin-resource': PLUGIN_RESOURCE_SCHEMA_VERSION,
  'plugin-diagnostic': PLUGIN_DIAGNOSTIC_SCHEMA_VERSION,
  'crash-capture': CRASH_CAPTURE_SCHEMA_VERSION,
  'release-manifest': RELEASE_MANIFEST_SCHEMA_VERSION,
  'release-transaction': RELEASE_TRANSACTION_SCHEMA_VERSION,
  'job-execution-environment': JOB_EXECUTION_ENVIRONMENT_SCHEMA_VERSION,
  operation: OPERATION_SCHEMA_VERSION,
  'portable-bundle': PORTABLE_BUNDLE_SCHEMA_VERSION,
  'import-plan': IMPORT_PLAN_SCHEMA_VERSION,
  'import-receipt': IMPORT_RECEIPT_SCHEMA_VERSION,
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
export class TauriCoreTransport implements SessionAwareCoreTransport, ActivationAwareCoreTransport {
  private readonly requestCommand: string;
  private readonly handshakeCommand: string;
  private readonly eventName: string;
  private readonly activationEventName: string;
  private readonly activationPendingCommand: string;
  private readonly clientFeatures: readonly string[];
  private readonly schemaVersions: Readonly<Record<string, number>>;
  private readonly clientId: string;
  private readonly onSequenceGap?: (expected: number, received: number) => void;
  private readonly onResync?: (snapshot: CoreAuthoritativeSnapshot) => void | Promise<void>;
  private nextRequestId = 1;
  private sessionId: string | undefined;
  private handshakeResponse: ProtocolHandshakeResponse | undefined;
  private handshakePromise: Promise<ProtocolHandshakeResponse> | undefined;
  private resyncPromise: Promise<CoreAuthoritativeSnapshot> | undefined;
  private lastEventSequence: number | undefined;
  private readonly listeners = new Set<(event: CoreEvent) => void>();
  private readonly statusListeners = new Set<(status: CoreTransportStatus) => void>();
  private readonly activationListeners = new Set<(request: CoreActivationRequest) => void>();
  private transportStatus: CoreTransportStatus = { state: 'disconnected' };
  private eventSubscription: Promise<(() => void)> | undefined;
  private activationSubscription: Promise<(() => void)> | undefined;
  private activationDrainPromise: Promise<void> | undefined;
  private disposed = false;

  public constructor(
    private readonly bridge: TauriBridge,
    options: TauriTransportOptions = {},
  ) {
    this.requestCommand = options.requestCommand ?? 'velvet_core_request';
    this.handshakeCommand = options.handshakeCommand ?? 'velvet_core_handshake';
    this.eventName = options.eventName ?? 'velvet_core_event';
    this.activationEventName = options.activationEventName ?? 'velvet_activation_request';
    this.activationPendingCommand = options.activationPendingCommand ?? 'velvet_activation_pending';
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
    if (this.transportStatus.state !== 'connected' && !isRecoverySafeRequest(request)) {
      throw new CoreTransportError({
        code: 'transport-recovering',
        message: 'The native core is recovering; mutation requests are temporarily read-only.',
        retryable: true,
      });
    }
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
        if (isTransportFailure(error.details)) {
          this.setStatus({ state: 'degraded', reason: error.details.message });
        }
        throw error;
      }

      const details = normalizeCoreError(error);
      if (isTransportFailure(details)) {
        this.setStatus({ state: 'degraded', reason: details.message });
      }
      throw new CoreTransportError(details);
    }
  }

  public subscribeStatus(listener: (status: CoreTransportStatus) => void): () => void {
    this.statusListeners.add(listener);
    try {
      listener(this.transportStatus);
    } catch {
      // A renderer status projection must not prevent the caller from
      // subscribing to future transport transitions.
    }
    return () => this.statusListeners.delete(listener);
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
    this.setStatus({ state: 'connecting' });
    this.handshakePromise = this.bridge
      .invoke<ProtocolHandshakeResponse>(this.handshakeCommand, { request })
      .then((response) => {
        this.validateHandshake(response, request);
        this.sessionId = response.sessionId;
        this.handshakeResponse = response;
        this.setStatus({ state: 'connected' });
        return response;
      })
      .catch((error) => {
        if (error instanceof CoreTransportError) {
          if (isTransportFailure(error.details)) {
            this.setStatus({ state: 'degraded', reason: error.details.message });
          }
          throw error;
        }
        const details = normalizeCoreError(error);
        if (isTransportFailure(details)) {
          this.setStatus({ state: 'degraded', reason: details.message });
        }
        throw new CoreTransportError(details);
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

  public subscribeActivation(listener: (request: CoreActivationRequest) => void): () => void {
    this.ensureUsable();
    this.activationListeners.add(listener);
    void this.ensureActivationSubscription().catch(() => undefined);
    return () => this.activationListeners.delete(listener);
  }

  public async dispose(): Promise<void> {
    if (this.disposed) {
      return;
    }

    this.disposed = true;
    this.setStatus({ state: 'disconnected' });
    this.listeners.clear();
    this.statusListeners.clear();
    this.activationListeners.clear();

    const unsubscribe = this.eventSubscription;
    this.eventSubscription = undefined;
    if (unsubscribe) {
      (await unsubscribe)();
    }
    const unsubscribeActivation = this.activationSubscription;
    this.activationSubscription = undefined;
    if (unsubscribeActivation) {
      (await unsubscribeActivation)();
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
        const details = normalizeCoreError(error);
        this.setStatus({ state: 'degraded', reason: details.message });
        throw new CoreTransportError(details);
      }
    }
  }

  private async ensureActivationSubscription(): Promise<void> {
    if (this.activationSubscription || this.disposed || this.activationListeners.size === 0) {
      return;
    }

    this.activationSubscription = this.bridge.listen<CoreActivationRequest>(
      this.activationEventName,
      ({ payload }) => this.handleActivation(payload),
    );

    try {
      const unsubscribe = await this.activationSubscription;
      if (this.disposed || this.activationListeners.size === 0) {
        unsubscribe();
        this.activationSubscription = undefined;
        return;
      }
      await this.drainActivationQueue();
    } catch (error) {
      this.activationSubscription = undefined;
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
      this.setStatus({ state: 'resyncing', reason: `event-sequence-gap:${expected}:${payload.sequence}` });
      try {
        this.onSequenceGap?.(expected, payload.sequence);
      } catch {
        // A diagnostic callback must not prevent the transport from resyncing.
      }
      if (!this.resyncPromise) {
        this.resyncPromise = this.reconnect()
          .then(() => this.resync())
          .then((snapshot) => {
            return Promise.resolve(this.onResync?.(snapshot)).then(() => snapshot);
          })
          .catch((error) => {
            const details = normalizeCoreError(error);
            this.setStatus({ state: 'degraded', reason: details.message });
            throw error;
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

  private handleActivation(payload: unknown): void {
    if (!isValidActivationRequest(payload)) {
      return;
    }
    void this.drainActivationQueue().catch(() => undefined);
  }

  private async drainActivationQueue(): Promise<void> {
    if (this.activationDrainPromise) {
      return this.activationDrainPromise;
    }
    this.activationDrainPromise = this.bridge
      .invoke<readonly CoreActivationRequest[]>(this.activationPendingCommand)
      .then((requests) => {
        if (!Array.isArray(requests)) {
          return;
        }
        for (const request of requests) {
          if (!isValidActivationRequest(request)) {
            continue;
          }
          for (const listener of this.activationListeners) {
            try {
              listener(request);
            } catch {
              // A consumer must not block subsequent activation requests.
            }
          }
        }
      })
      .finally(() => {
        this.activationDrainPromise = undefined;
      });
    return this.activationDrainPromise;
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

  private setStatus(status: CoreTransportStatus): void {
    if (
      this.transportStatus.state === status.state &&
      this.transportStatus.reason === status.reason
    ) {
      return;
    }
    this.transportStatus = status;
    for (const listener of this.statusListeners) {
      try {
        listener(status);
      } catch {
        // A renderer status projection must not break transport recovery.
      }
    }
  }
}

function isValidActivationRequest(value: unknown): value is CoreActivationRequest {
  if (typeof value !== 'object' || value === null) {
    return false;
  }
  const candidate = value as {
    schemaVersion?: unknown;
    requestId?: unknown;
    targets?: unknown;
  };
  if (
    candidate.schemaVersion !== ACTIVATION_SCHEMA_VERSION ||
    typeof candidate.requestId !== 'string' ||
    candidate.requestId.length === 0 ||
    candidate.requestId.length > 128 ||
    !Array.isArray(candidate.targets) ||
    candidate.targets.length > 32
  ) {
    return false;
  }
  return candidate.targets.every((target) => {
    if (typeof target !== 'object' || target === null) {
      return false;
    }
    const activationTarget = target as { kind?: unknown; value?: unknown };
    if (
      (activationTarget.kind !== 'file-path' && activationTarget.kind !== 'deep-link') ||
      typeof activationTarget.value !== 'string' ||
      activationTarget.value.length === 0 ||
      activationTarget.value.length > 4096 ||
      activationTarget.value.includes('\0')
    ) {
      return false;
    }
    return (
      activationTarget.kind !== 'deep-link' ||
      activationTarget.value.startsWith('velvet://')
    );
  });
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

function isRecoverySafeRequest(request: CoreRequest): boolean {
  return request.kind.startsWith('query-') || [
    'read-plugin-localization',
    'inspect-plugin',
    'inspect-plugin-archive',
    'release-preview-session',
  ].includes(request.kind);
}

function isTransportFailure(error: CoreError): boolean {
  return [
    'transport-error',
    'request-id-mismatch',
    'session-expired',
    'session-required',
    'core-lock-poisoned',
    'protocol-version-mismatch',
    'protocol-major-mismatch',
    'invalid-handshake',
    'invalid-resync-response',
    'invalid-request-outcome-response',
  ].includes(error.code);
}
