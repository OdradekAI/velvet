import {
  PLUGIN_RESOURCE_SCHEMA_VERSION,
  type CorePluginLocalization,
  type CoreTransport,
} from '../../platform/protocol.js';
import type {
  LocalizationBundleInput,
  LocalizationBundleLoader,
} from './localization-service.js';

/**
 * Loads package-relative localization bundles through the typed core seam.
 * The renderer receives bounded bytes only; package paths and filesystem
 * checks remain owned by the Rust PluginResourceLoader.
 */
export class CoreLocalizationBundleLoader implements LocalizationBundleLoader {
  public constructor(private readonly core: CoreTransport) {}

  public async load(
    extensionId: string,
    manifest: CorePluginLocalization,
  ): Promise<readonly LocalizationBundleInput[]> {
    const bundles = await Promise.all(
      manifest.supportedLocales.map(async (locale) => {
        try {
          const response = await this.core.request({
            protocolVersion: 1,
            kind: 'read-plugin-localization',
            extensionId,
            locale,
          });
          if (response.kind !== 'plugin-resource') {
            throw new Error(`Expected a plugin resource response, received ${response.kind}`);
          }
          const resource = response.resource;
          if (
            resource.schemaVersion !== PLUGIN_RESOURCE_SCHEMA_VERSION ||
            resource.extensionId !== extensionId ||
            resource.locale !== locale ||
            resource.contentType !== 'application/json'
          ) {
            throw new Error(`Plugin resource metadata is invalid for ${extensionId}/${locale}`);
          }
          const text = new TextDecoder('utf-8', { fatal: true }).decode(
            Uint8Array.from(resource.bytes),
          );
          const messages: unknown = JSON.parse(text);
          if (!messages || typeof messages !== 'object' || Array.isArray(messages)) {
            throw new Error(`Plugin localization bundle must be a JSON object for ${locale}`);
          }
          return { locale, messages: messages as Readonly<Record<string, unknown>> };
        } catch (error) {
          if (isMissingPluginResource(error)) {
            return undefined;
          }
          throw error;
        }
      }),
    );
    return bundles.filter((bundle): bundle is LocalizationBundleInput => bundle !== undefined);
  }
}

function isMissingPluginResource(error: unknown): boolean {
  if (!error || typeof error !== 'object' || !('details' in error)) {
    return false;
  }
  const details = (error as { details?: unknown }).details;
  return Boolean(
    details &&
      typeof details === 'object' &&
      'code' in details &&
      (details as { code?: unknown }).code === 'plugin-resource-not-found',
  );
}
