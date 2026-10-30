import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { VelvetDialog } from '../ui/velvet-ui.js';
import {
  PreviewHost,
  type PreviewContext,
  type ResolvedPreview,
} from '../../workbench/preview/preview-host.js';
import type {
  CorePreviewActionContext,
  CoreResponse,
  CoreResolvedPreviewAugmentation,
  CorePreviewSession,
} from '../../platform/protocol.js';
import { PreviewRegistry } from '../../workbench/preview/preview-registry.js';
import { PreviewRendererRegistry } from '../../workbench/preview/preview-renderer-registry.js';
import { PreviewRendererHost } from './preview-renderer-host.js';
import { BuiltinPreviewRenderer } from './builtin-preview-renderer.js';
import type { WorkspaceContext } from '../../workbench/services/workspace-context.js';
import { CommandRegistry } from '../../workbench/commands/command-registry.js';
import { ContextKeyService } from '../../workbench/commands/context-key-service.js';
import { JobClient } from '../../workbench/jobs/job-client.js';

export interface PreviewOverlayProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly host: PreviewHost;
  readonly registry: PreviewRegistry;
  readonly rendererRegistry?: PreviewRendererRegistry;
  readonly workspaceContext?: WorkspaceContext;
  readonly commands: CommandRegistry;
  readonly contextKeys: ContextKeyService;
  readonly jobs: JobClient;
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
  commands,
  contextKeys,
  jobs,
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
  const resolvedAugmentations = snapshot.preview?.augmentations ?? [];
  const augmentations = resolvedAugmentations.length > 0
    ? resolvedAugmentations
    : registry.listAugmentations(context.contentKind).map((augmentation) => ({
        augmentationId: augmentation.id,
        slot: augmentation.slot,
        session: null,
        diagnostic: null,
      }));

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
              ) : snapshot.preview.session.renderer.kind === 'text' ||
                snapshot.preview.session.renderer.kind === 'image' ? (
                <BuiltinPreviewRenderer renderer={snapshot.preview.session.renderer} />
              ) : (
                <>
                  <strong>{snapshot.preview.session.providerId}</strong>
                  <p>Asset {snapshot.preview.session.context.assetId}</p>
                </>
              )
            )}
            <PreviewSessionActions
              commands={commands}
              contextKeys={contextKeys}
              jobs={jobs}
              session={snapshot.preview.session}
              workspaceContext={workspaceContext}
            />
          </div>
        ) : null}
        {snapshot.error ? (
          <p aria-label="Preview diagnostic" data-preview-error>{snapshot.error}</p>
        ) : null}
        {augmentations.length > 0 ? (
          <div data-preview-augmentations>
            {augmentations.map((augmentation) => (
              <div
                data-preview-augmentation={augmentation.augmentationId}
                data-preview-slot={augmentation.slot}
                key={augmentation.augmentationId}
              >
                <strong>{augmentation.augmentationId}</strong>
                {augmentation.diagnostic ? (
                  <p data-preview-augmentation-error role="status">
                    {augmentation.diagnostic}
                  </p>
                ) : null}
                {augmentation.session ? (
                  <PreviewAugmentationSession
                    augmentation={augmentation}
                    basePreview={snapshot.preview}
                    context={workspaceContext}
                    commands={commands}
                    contextKeys={contextKeys}
                    jobs={jobs}
                    rendererRegistry={rendererRegistry}
                  />
                ) : (
                  <p data-preview-augmentation-state="waiting" role="status">
                    Waiting for the augmentation renderer.
                  </p>
                )}
              </div>
            ))}
          </div>
        ) : null}
      </section>
    </VelvetDialog>
  );
}

function PreviewAugmentationSession({
  augmentation,
  basePreview,
  context,
  commands,
  contextKeys,
  jobs,
  rendererRegistry,
}: {
  readonly augmentation: CoreResolvedPreviewAugmentation;
  readonly basePreview?: ResolvedPreview;
  readonly context?: WorkspaceContext;
  readonly commands: CommandRegistry;
  readonly contextKeys: ContextKeyService;
  readonly jobs: JobClient;
  readonly rendererRegistry?: PreviewRendererRegistry;
}) {
  if (!augmentation.session) {
    return null;
  }
  const preview = basePreview
    ? {
        ...basePreview,
        candidate: {
          ...basePreview.candidate,
          providerId: augmentation.augmentationId,
        },
        session: augmentation.session,
        diagnostic: augmentation.diagnostic,
        augmentations: [],
      }
    : undefined;
  if (!preview) {
    return null;
  }

  let renderer: ReactNode = undefined;
  if (augmentation.session.renderer.kind === 'ui-bridge' && rendererRegistry && context) {
    renderer = (
      <PreviewRendererHost
        context={context}
        preview={preview}
        registry={rendererRegistry}
      />
    );
  } else if (
    augmentation.session.renderer.kind === 'text' ||
    augmentation.session.renderer.kind === 'image'
  ) {
    renderer = <BuiltinPreviewRenderer renderer={augmentation.session.renderer} />;
  }

  return (
    <>
      {renderer ?? <p data-preview-augmentation-state="ready">Augmentation session ready.</p>}
      <PreviewSessionActions
        commands={commands}
        contextKeys={contextKeys}
        jobs={jobs}
        session={augmentation.session}
        workspaceContext={context}
      />
    </>
  );
}

function PreviewSessionActions({
  commands,
  contextKeys,
  jobs,
  session,
  workspaceContext,
}: {
  readonly commands: CommandRegistry;
  readonly contextKeys: ContextKeyService;
  readonly jobs: JobClient;
  readonly session: CorePreviewSession;
  readonly workspaceContext?: WorkspaceContext;
}) {
  if (session.commands.length === 0) {
    return null;
  }
  return (
    <div data-preview-actions>
      {session.commands.map((commandId) => (
        <PreviewActionButton
          commandId={commandId}
          commands={commands}
          contextKeys={contextKeys}
          jobs={jobs}
          key={commandId}
          session={session}
          workspaceContext={workspaceContext}
        />
      ))}
    </div>
  );
}

function PreviewActionButton({
  commandId,
  commands,
  contextKeys,
  jobs,
  session,
  workspaceContext,
}: {
  readonly commandId: string;
  readonly commands: CommandRegistry;
  readonly contextKeys: ContextKeyService;
  readonly jobs: JobClient;
  readonly session: CorePreviewSession;
  readonly workspaceContext?: WorkspaceContext;
}) {
  const [, rerender] = useState(0);
  const [state, setState] = useState<'idle' | 'running' | 'accepted' | 'failed'>('idle');
  const [error, setError] = useState<string>();
  const [jobId, setJobId] = useState<string>();
  const activeAppId = workspaceContext?.snapshot().activeAppId;
  const command = commands.resolve(commandId, activeAppId);
  const job = jobId ? jobs.get(jobId) : undefined;

  useEffect(() => jobs.subscribe(() => rerender((value) => value + 1)), [jobs]);

  if (!command) {
    return (
      <span data-preview-action-state="unavailable" data-preview-action={commandId}>
        {commandId} unavailable
      </span>
    );
  }

  const actionContext: CorePreviewActionContext = {
    sessionId: session.sessionId,
    providerId: session.providerId,
    assetId: session.context.assetId,
    contentKind: session.context.contentKind,
    intent: session.context.intent,
    surface: session.context.surface,
  };

  async function execute(): Promise<void> {
    setState('running');
    setError(undefined);
    try {
      const result = await commands.execute(
        commandId,
        { preview: actionContext },
        {
          activeAppId,
          keys: contextKeys.snapshot(),
          preview: actionContext,
        },
      );
      if (isJobResponse(result)) {
        jobs.accept(result.job);
        setJobId(result.job.id);
      }
      setState('accepted');
    } catch (cause) {
      setState('failed');
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }

  return (
    <div data-preview-action={commandId} data-preview-action-state={state}>
      <button disabled={state === 'running'} onClick={() => void execute()} type="button">
        {command.title}
      </button>
      {error ? <span data-preview-action-error role="alert">{error}</span> : null}
      {job ? (
        <span data-preview-action-job={job.id} role="status">
          {job.status} · {job.progress}%
        </span>
      ) : null}
    </div>
  );
}

function isJobResponse(value: unknown): value is Extract<CoreResponse, { readonly kind: 'job' }> {
  return Boolean(value && typeof value === 'object' && 'kind' in value && value.kind === 'job');
}
