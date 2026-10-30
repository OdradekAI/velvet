import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { VelvetDialog } from '../ui/velvet-ui.js';
import {
  PreviewHost,
  type PreviewContext,
  type ResolvedPreview,
} from '../../workbench/preview/preview-host.js';
import { PreviewRegistry } from '../../workbench/preview/preview-registry.js';
import { PreviewRendererRegistry } from '../../workbench/preview/preview-renderer-registry.js';
import { PreviewRendererHost } from './preview-renderer-host.js';
import type { WorkspaceContext } from '../../workbench/services/workspace-context.js';

export interface PreviewOverlayProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly host: PreviewHost;
  readonly registry: PreviewRegistry;
  readonly rendererRegistry?: PreviewRendererRegistry;
  readonly workspaceContext?: WorkspaceContext;
  readonly context: PreviewContext;
  readonly primaryRenderer?: (preview: ResolvedPreview) => ReactNode;
}

/** Host-owned Quick preview frame with extension-owned primary and augmentation slots. */
export function PreviewOverlay({
  open,
  onOpenChange,
  host,
  registry,
  rendererRegistry,
  workspaceContext,
  context,
  primaryRenderer,
}: PreviewOverlayProps) {
  const [, rerender] = useState(0);

  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const unsubscribe = host.subscribe(() => rerender((value) => value + 1));
    void host.open(context);
    return () => {
      unsubscribe();
      host.close();
    };
  }, [context, host, open]);

  const snapshot = host.snapshot;
  const augmentations = registry.listAugmentations(context.contentKind);

  return (
    <VelvetDialog
      description="The host owns this preview frame; providers and augmentations fill declared slots."
      onOpenChange={(nextOpen) => {
        if (!nextOpen) {
          host.close();
        }
        onOpenChange(nextOpen);
      }}
      open={open}
      title={`Preview · ${context.contentKind}`}
    >
      <section aria-live="polite" data-preview-host-state={snapshot.state}>
        {snapshot.state === 'resolving' || snapshot.state === 'loading' ? (
          <p data-preview-loading>Resolving preview…</p>
        ) : null}
        {snapshot.state === 'failed' ? (
          <p aria-label="Preview fallback" data-preview-fallback role="status">
            Preview provider failed. Showing the generic fallback.
          </p>
        ) : null}
        {snapshot.preview ? (
          <div data-preview-primary>
            {snapshot.preview.diagnostic ? (
              <p aria-label="Preview diagnostic" data-preview-diagnostic role="status">
                {snapshot.preview.diagnostic}
              </p>
            ) : null}
            {primaryRenderer?.(snapshot.preview) ?? (
              snapshot.preview.session.renderer.kind === 'ui-bridge' &&
              rendererRegistry &&
              workspaceContext ? (
                <PreviewRendererHost
                  context={workspaceContext}
                  preview={snapshot.preview}
                  registry={rendererRegistry}
                />
              ) : (
                <>
                  <strong>{snapshot.preview.session.providerId}</strong>
                  <p>Asset {snapshot.preview.session.context.assetId}</p>
                </>
              )
            )}
          </div>
        ) : null}
        {snapshot.error ? (
          <p aria-label="Preview diagnostic" data-preview-error>{snapshot.error}</p>
        ) : null}
        {augmentations.length > 0 ? (
          <div data-preview-augmentations>
            {augmentations.map((augmentation) => (
              <div
                data-preview-augmentation={augmentation.id}
                data-preview-slot={augmentation.slot}
                key={augmentation.id}
              >
                {augmentation.id}
              </div>
            ))}
          </div>
        ) : null}
      </section>
    </VelvetDialog>
  );
}
