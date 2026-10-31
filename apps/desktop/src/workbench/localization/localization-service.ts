import type {
  CorePluginLocalization,
  CoreProfileState,
  CoreTransport,
} from '../../platform/protocol.js';
import { PROFILE_SCHEMA_VERSION } from '../../platform/protocol.js';

export type LocalizationValue = string | number | boolean | Date;
export type LocalizationArguments = Readonly<Record<string, LocalizationValue>>;

export interface LocalizationBundleInput {
  readonly locale: string;
  readonly messages: Readonly<Record<string, unknown>>;
}

export interface LocalizationBundle {
  readonly locale: string;
  readonly messages: Readonly<Record<string, string>>;
}

export interface LocalizationDiagnostic {
  readonly extensionId: string;
  readonly locale?: string;
  readonly messageId?: string;
  readonly code:
    | 'invalid-manifest'
    | 'invalid-locale'
    | 'missing-default-bundle'
    | 'invalid-bundle'
    | 'invalid-message';
  readonly message: string;
}

export interface LocalizationBundleLoader {
  load(
    extensionId: string,
    manifest: CorePluginLocalization,
  ): Promise<readonly LocalizationBundleInput[]>;
}

export interface LocalizationHandle {
  readonly extensionId: string;
  readonly locale: string;
  readonly revision: number;
  t(
    messageId: string,
    defaultMessage: string,
    args?: LocalizationArguments,
  ): string;
  subscribe(listener: () => void): () => void;
}

export interface LocalizationSnapshot {
  readonly locale: string;
  readonly revision: number;
}

export interface LocalizationProjection {
  readonly extensionId: string;
  readonly locale: string;
  readonly revision: number;
  readonly messages: Readonly<Record<string, string>>;
}

interface RegisteredLocalization {
  readonly extensionId: string;
  readonly manifest: CorePluginLocalization;
  readonly bundles: ReadonlyMap<string, LocalizationBundle>;
}

const LOCALE_PATTERN = /^[A-Za-z]{2,8}(?:[-_][A-Za-z0-9]{1,8})*$/;
const MESSAGE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*$/;

/**
 * Host-owned runtime for extension messages. The service intentionally knows
 * nothing about React, Vue, Tauri or package paths; a loader supplies bundle
 * bytes and a UI adapter projects the handle into its own rendering model.
 */
export class LocalizationService {
  private readonly extensions = new Map<string, RegisteredLocalization>();
  private readonly listeners = new Set<() => void>();
  private readonly diagnostics: LocalizationDiagnostic[] = [];
  private currentLocale: string;
  private currentRevision = 0;
  private coreUnsubscribe: (() => void) | undefined;

  public constructor(initialLocale = 'en') {
    this.currentLocale = normalizeLocale(initialLocale) ?? 'en';
  }

  public get snapshot(): LocalizationSnapshot {
    return { locale: this.currentLocale, revision: this.currentRevision };
  }

  public getDiagnostics(): readonly LocalizationDiagnostic[] {
    return [...this.diagnostics];
  }

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public setLocale(locale: string): boolean {
    const normalized = normalizeLocale(locale);
    if (!normalized) {
      this.recordDiagnostic({
        extensionId: 'host',
        code: 'invalid-locale',
        message: `Invalid locale: ${locale}`,
      });
      return false;
    }
    if (normalized === this.currentLocale) {
      return true;
    }

    this.currentLocale = normalized;
    this.currentRevision += 1;
    this.notify();
    return true;
  }

  /** Binds the host projection so Profile/Session locale remains authoritative. */
  public bindCore(core: CoreTransport): void {
    this.coreUnsubscribe?.();
    this.coreUnsubscribe = core.subscribe((event) => {
      if (event.kind === 'profile-changed') {
        this.applyProfile(event.state);
      }
    });
  }

  public async loadFromCore(core: CoreTransport): Promise<LocalizationSnapshot> {
    const response = await core.request({
      protocolVersion: 1,
      kind: 'query-profile',
    });
    if (response.kind !== 'profile') {
      throw new Error(`Expected a profile response, received ${response.kind}`);
    }
    this.applyProfile(response.state);
    return this.snapshot;
  }

  public async setLocaleInCore(core: CoreTransport, locale: string): Promise<boolean> {
    const normalized = normalizeLocale(locale);
    if (!normalized) {
      return this.setLocale(locale);
    }
    const response = await core.request({
      protocolVersion: 1,
      kind: 'set-locale',
      locale: normalized,
      expectedRevision: this.currentRevision,
    });
    if (response.kind !== 'profile') {
      throw new Error(`Expected a profile response, received ${response.kind}`);
    }
    return this.applyProfile(response.state);
  }

  /** Registers valid bundles and keeps a broken bundle local to its extension. */
  public registerExtension(
    extensionId: string,
    manifest: CorePluginLocalization,
    bundles: readonly LocalizationBundleInput[],
  ): boolean {
    const normalizedManifest = validateManifest(extensionId, manifest, (diagnostic) => {
      this.recordDiagnostic(diagnostic);
    });

    const parsedBundles = new Map<string, LocalizationBundle>();
    for (const input of bundles) {
      let bundle: LocalizationBundle | undefined;
      try {
        bundle = parseBundle(extensionId, normalizedManifest, input, (diagnostic) => {
          this.recordDiagnostic(diagnostic);
        });
      } catch (cause) {
        this.recordDiagnostic({
          extensionId,
          code: 'invalid-bundle',
          message: `Localization bundle is malformed: ${formatCause(cause)}`,
        });
      }
      if (bundle) {
        if (parsedBundles.has(bundle.locale)) {
          this.recordDiagnostic({
            extensionId,
            locale: bundle.locale,
            code: 'invalid-bundle',
            message: `Duplicate localization bundle for ${bundle.locale}`,
          });
        } else {
          parsedBundles.set(bundle.locale, bundle);
        }
      }
    }

    if (!parsedBundles.has(normalizedManifest.defaultLocale)) {
      this.recordDiagnostic({
        extensionId,
        locale: normalizedManifest.defaultLocale,
        code: 'missing-default-bundle',
        message: `Default localization bundle is missing for ${extensionId}`,
      });
    }

    this.extensions.set(extensionId, {
      extensionId,
      manifest: normalizedManifest,
      bundles: parsedBundles,
    });
    this.notify();
    return true;
  }

  public async loadExtension(
    extensionId: string,
    manifest: CorePluginLocalization,
    loader: LocalizationBundleLoader,
  ): Promise<boolean> {
    try {
      const bundles = await loader.load(extensionId, manifest);
      return this.registerExtension(extensionId, manifest, bundles);
    } catch (cause) {
      this.recordDiagnostic({
        extensionId,
        code: 'invalid-bundle',
        message: `Localization bundle loading failed: ${formatCause(cause)}`,
      });
      // Keep the extension addressable so defaultMessage remains usable.
      return this.registerExtension(extensionId, manifest, []);
    }
  }

  public unregisterExtension(extensionId: string): boolean {
    const removed = this.extensions.delete(extensionId);
    if (removed) {
      this.notify();
    }
    return removed;
  }

  public dispose(): void {
    this.coreUnsubscribe?.();
    this.coreUnsubscribe = undefined;
    this.listeners.clear();
  }

  /** Returns the selected bundle for UI Bridge projection, if one is loaded. */
  public projection(extensionId: string): LocalizationProjection | undefined {
    const registration = this.extensions.get(extensionId);
    if (!registration) {
      return undefined;
    }
    const locale = localeFallbacks(
      this.currentLocale,
      registration.manifest.defaultLocale,
    ).find((candidate) => registration.bundles.has(candidate));
    const bundle = locale ? registration.bundles.get(locale) : undefined;
    return {
      extensionId,
      locale: locale ?? registration.manifest.defaultLocale,
      revision: this.currentRevision,
      messages: bundle?.messages ?? {},
    };
  }

  public handle(extensionId: string): LocalizationHandle {
    const service = this;
    return {
      extensionId,
      get locale() {
        return service.currentLocale;
      },
      get revision() {
        return service.currentRevision;
      },
      t: (messageId, defaultMessage, args) =>
        this.t(extensionId, messageId, defaultMessage, args),
      subscribe: (listener) => service.subscribe(listener),
    };
  }

  public t(
    extensionId: string,
    messageId: string,
    defaultMessage: string,
    args: LocalizationArguments = {},
  ): string {
    const fullMessageId = qualifyMessageId(extensionId, messageId);
    if (!defaultMessage) {
      this.recordDiagnostic({
        extensionId,
        messageId: fullMessageId,
        code: 'invalid-message',
        message: 'defaultMessage must not be empty',
      });
      return '';
    }

    const registration = this.extensions.get(extensionId);
    const translation = registration
      ? this.findTranslation(registration, fullMessageId)
      : undefined;
    const selected = translation ?? defaultMessage;

    try {
      const defaultVariables = collectVariables(defaultMessage);
      const selectedVariables = collectVariables(selected);
      if (!sameVariables(defaultVariables, selectedVariables)) {
        throw new Error('translation placeholders do not match defaultMessage');
      }
      return formatMessage(selected, args, this.currentLocale);
    } catch (cause) {
      this.recordDiagnostic({
        extensionId,
        locale: this.currentLocale,
        messageId: fullMessageId,
        code: 'invalid-message',
        message: `Message fell back to defaultMessage: ${formatCause(cause)}`,
      });
      try {
        return formatMessage(defaultMessage, args, this.currentLocale);
      } catch {
        return defaultMessage;
      }
    }
  }

  private findTranslation(
    registration: RegisteredLocalization,
    messageId: string,
  ): string | undefined {
    const locales = localeFallbacks(
      this.currentLocale,
      registration.manifest.defaultLocale,
    );
    for (const locale of locales) {
      const bundle = registration.bundles.get(locale);
      const value = bundle?.messages[messageId];
      if (value !== undefined) {
        return value;
      }
    }
    return undefined;
  }

  private recordDiagnostic(diagnostic: LocalizationDiagnostic): void {
    this.diagnostics.push(diagnostic);
    if (this.diagnostics.length > 200) {
      this.diagnostics.splice(0, this.diagnostics.length - 200);
    }
    this.notify();
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // One extension's localization projection must not block the host.
      }
    }
  }

  private applyProfile(state: CoreProfileState): boolean {
    const locale = normalizeLocale(state.locale);
    if (
      state.schemaVersion !== PROFILE_SCHEMA_VERSION ||
      !locale ||
      state.revision < this.currentRevision
    ) {
      return false;
    }
    const changed = locale !== this.currentLocale || state.revision !== this.currentRevision;
    this.currentLocale = locale;
    this.currentRevision = state.revision;
    if (changed) {
      this.notify();
    }
    return true;
  }
}

function validateManifest(
  extensionId: string,
  manifest: CorePluginLocalization,
  report: (diagnostic: LocalizationDiagnostic) => void,
): CorePluginLocalization {
  const defaultLocale = normalizeLocale(manifest.defaultLocale);
  const supportedLocales = manifest.supportedLocales
    .map(normalizeLocale)
    .filter((locale): locale is string => locale !== undefined);
  const valid = Boolean(
    defaultLocale &&
      supportedLocales.length > 0 &&
      supportedLocales.includes(defaultLocale) &&
      manifest.resourceRoot.trim() &&
      !manifest.resourceRoot.startsWith('/') &&
      !manifest.resourceRoot.split('/').some((segment) => segment === '..') &&
      manifest.bundleSchema > 0,
  );

  if (!valid) {
    report({
      extensionId,
      code: 'invalid-manifest',
      message: 'Localization metadata is invalid',
    });
  }

  return {
    ...manifest,
    defaultLocale: defaultLocale ?? 'en',
    supportedLocales: supportedLocales.length > 0 ? supportedLocales : ['en'],
  };
}

function parseBundle(
  extensionId: string,
  manifest: CorePluginLocalization,
  input: LocalizationBundleInput,
  report: (diagnostic: LocalizationDiagnostic) => void,
): LocalizationBundle | undefined {
  if (!input || typeof input !== 'object' || !input.messages || typeof input.messages !== 'object' || Array.isArray(input.messages)) {
    report({
      extensionId,
      code: 'invalid-bundle',
      message: 'Localization bundle must contain an object of messages',
    });
    return undefined;
  }
  const locale = normalizeLocale(input.locale);
  if (!locale || !manifest.supportedLocales.includes(locale)) {
    report({
      extensionId,
      locale: input.locale,
      code: 'invalid-locale',
      message: `Bundle locale is not supported: ${input.locale}`,
    });
    return undefined;
  }

  const messages: Record<string, string> = {};
  for (const [key, value] of Object.entries(input.messages)) {
    const messageId = qualifyMessageId(extensionId, key);
    if (!MESSAGE_ID_PATTERN.test(key) || typeof value !== 'string') {
      report({
        extensionId,
        locale,
        messageId,
        code: 'invalid-bundle',
        message: `Invalid localization message ${key}`,
      });
      continue;
    }
    if (messageId in messages) {
      report({
        extensionId,
        locale,
        messageId,
        code: 'invalid-bundle',
        message: `Duplicate localization message ${messageId}`,
      });
      continue;
    }
    messages[messageId] = value;
  }

  return { locale, messages };
}

function normalizeLocale(locale: string): string | undefined {
  if (!LOCALE_PATTERN.test(locale)) {
    return undefined;
  }
  try {
    return new Intl.Locale(locale.replace('_', '-')).toString();
  } catch {
    return undefined;
  }
}

function localeFallbacks(requested: string, defaultLocale: string): readonly string[] {
  const normalizedRequested = normalizeLocale(requested) ?? requested;
  const language = normalizedRequested.split('-')[0];
  return [...new Set([normalizedRequested, language, defaultLocale])];
}

function qualifyMessageId(extensionId: string, messageId: string): string {
  return messageId.startsWith(`${extensionId}.`) ? messageId : `${extensionId}.${messageId}`;
}

function sameVariables(left: ReadonlySet<string>, right: ReadonlySet<string>): boolean {
  if (left.size !== right.size) {
    return false;
  }
  for (const variable of left) {
    if (!right.has(variable)) {
      return false;
    }
  }
  return true;
}

function collectVariables(message: string): ReadonlySet<string> {
  const variables = new Set<string>();
  visitMessage(message, (expression) => {
    const parts = splitExpression(expression);
    const name = parts[0]?.trim();
    if (!name) {
      throw new Error('empty message placeholder');
    }
    variables.add(name);
    if (parts[1]?.trim() === 'plural' || parts[1]?.trim() === 'select') {
      for (const option of parseOptions(parts.slice(2).join(',').trim())) {
        collectInto(option.body, variables);
      }
    }
  });
  return variables;
}

function collectInto(message: string, variables: Set<string>): void {
  for (const variable of collectVariables(message)) {
    variables.add(variable);
  }
}

function formatMessage(
  message: string,
  args: LocalizationArguments,
  locale: string,
): string {
  return renderMessage(message, args, locale);
}

function renderMessage(
  message: string,
  args: LocalizationArguments,
  locale: string,
  pluralValue?: number,
): string {
  let result = '';
  let cursor = 0;
  while (cursor < message.length) {
    const character = message[cursor];
    if (character === '#') {
      result += pluralValue === undefined ? '#' : new Intl.NumberFormat(locale).format(pluralValue);
      cursor += 1;
      continue;
    }
    if (character !== '{') {
      result += character;
      cursor += 1;
      continue;
    }

    const end = matchingBrace(message, cursor);
    const expression = message.slice(cursor + 1, end);
    const parts = splitExpression(expression);
    const name = parts[0]?.trim();
    if (!name) {
      throw new Error('empty message placeholder');
    }
    const value = args[name];
    const kind = parts[1]?.trim();
    if (!kind) {
      if (value === undefined) {
        throw new Error(`missing argument: ${name}`);
      }
      result += String(value);
    } else if (kind === 'number' || kind === 'date' || kind === 'time') {
      result += formatTypedValue(value, kind, locale, name);
    } else if (kind === 'select') {
      if (value === undefined) {
        throw new Error(`missing argument: ${name}`);
      }
      const options = parseOptions(parts.slice(2).join(',').trim());
      const selected = options.find((option) => option.key === String(value))
        ?? options.find((option) => option.key === 'other');
      if (!selected) {
        throw new Error(`select has no other branch: ${name}`);
      }
      result += renderMessage(selected.body, args, locale, pluralValue);
    } else if (kind === 'plural') {
      const numeric = toNumber(value, name);
      const options = parseOptions(parts.slice(2).join(',').trim());
      const exact = options.find((option) => option.key === `=${numeric}`);
      const category = new Intl.PluralRules(locale).select(numeric);
      const selected = exact
        ?? options.find((option) => option.key === category)
        ?? options.find((option) => option.key === 'other');
      if (!selected) {
        throw new Error(`plural has no other branch: ${name}`);
      }
      result += renderMessage(selected.body, args, locale, numeric);
    } else {
      throw new Error(`unsupported message format: ${kind}`);
    }
    cursor = end + 1;
  }
  return result;
}

function formatTypedValue(
  value: LocalizationValue | undefined,
  kind: string,
  locale: string,
  name: string,
): string {
  if (value === undefined) {
    throw new Error(`missing argument: ${name}`);
  }
  if (kind === 'number') {
    return new Intl.NumberFormat(locale).format(toNumber(value, name));
  }
  const date = value instanceof Date
    ? value
    : typeof value === 'string' || typeof value === 'number'
      ? new Date(value)
      : new Date(Number.NaN);
  if (Number.isNaN(date.valueOf())) {
    throw new Error(`invalid date argument: ${name}`);
  }
  return kind === 'time'
    ? new Intl.DateTimeFormat(locale, { timeStyle: 'short' }).format(date)
    : new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(date);
}

function toNumber(value: LocalizationValue | undefined, name: string): number {
  const numeric = typeof value === 'number' ? value : Number(value);
  if (!Number.isFinite(numeric)) {
    throw new Error(`invalid numeric argument: ${name}`);
  }
  return numeric;
}

function visitMessage(message: string, visitor: (expression: string) => void): void {
  let cursor = 0;
  while (cursor < message.length) {
    const start = message.indexOf('{', cursor);
    if (start < 0) {
      return;
    }
    const end = matchingBrace(message, start);
    const expression = message.slice(start + 1, end);
    visitor(expression);
    const parts = splitExpression(expression);
    if (parts[1]?.trim() === 'plural' || parts[1]?.trim() === 'select') {
      for (const option of parseOptions(parts.slice(2).join(',').trim())) {
        visitMessage(option.body, visitor);
      }
    }
    cursor = end + 1;
  }
}

function matchingBrace(message: string, start: number): number {
  let depth = 0;
  for (let index = start; index < message.length; index += 1) {
    if (message[index] === '{') {
      depth += 1;
    } else if (message[index] === '}') {
      depth -= 1;
      if (depth === 0) {
        return index;
      }
    }
  }
  throw new Error('unclosed message placeholder');
}

function splitExpression(expression: string): string[] {
  const parts: string[] = [];
  let start = 0;
  let depth = 0;
  for (let index = 0; index < expression.length; index += 1) {
    const character = expression[index];
    if (character === '{') {
      depth += 1;
    } else if (character === '}') {
      depth -= 1;
    } else if (character === ',' && depth === 0) {
      parts.push(expression.slice(start, index));
      start = index + 1;
    }
  }
  parts.push(expression.slice(start));
  return parts;
}

interface MessageOption {
  readonly key: string;
  readonly body: string;
}

function parseOptions(expression: string): readonly MessageOption[] {
  const options: MessageOption[] = [];
  let cursor = 0;
  while (cursor < expression.length) {
    while (cursor < expression.length && /\s/.test(expression[cursor])) {
      cursor += 1;
    }
    const keyStart = cursor;
    while (cursor < expression.length && !/\s|\{/.test(expression[cursor])) {
      cursor += 1;
    }
    const key = expression.slice(keyStart, cursor);
    while (cursor < expression.length && /\s/.test(expression[cursor])) {
      cursor += 1;
    }
    if (!key || expression[cursor] !== '{') {
      throw new Error('invalid plural/select option');
    }
    const end = matchingBrace(expression, cursor);
    options.push({ key, body: expression.slice(cursor + 1, end) });
    cursor = end + 1;
  }
  return options;
}

function formatCause(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
