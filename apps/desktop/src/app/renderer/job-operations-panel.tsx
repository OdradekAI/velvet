import { useCallback, useEffect, useState } from 'react';
import type { CoreJobSnapshot } from '../../platform/protocol.js';
import type { JobClient } from '../../workbench/jobs/job-client.js';
import { VelvetBadge, VelvetButton, VelvetProgress, VelvetState, VelvetText } from '../ui/velvet-ui.js';

export interface JobOperationsPanelProps {
  readonly jobs: JobClient;
}

function statusLabel(status: CoreJobSnapshot['status']): string {
  return status[0].toUpperCase() + status.slice(1);
}

/** Renders the global JobSystem projection and host-owned lifecycle actions. */
export function JobOperationsPanel({ jobs }: JobOperationsPanelProps) {
  const [items, setItems] = useState<readonly CoreJobSnapshot[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      setError(undefined);
      await jobs.query();
      setItems(jobs.list());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    }
  }, [jobs]);

  useEffect(() => {
    void refresh();
    const unsubscribe = jobs.subscribe(() => setItems(jobs.list()));
    const interval = window.setInterval(() => void refresh(), 350);
    return () => {
      unsubscribe();
      window.clearInterval(interval);
    };
  }, [jobs, refresh]);

  async function create(kind: string, willFail: boolean, durationMs = 2_400): Promise<void> {
    setBusy(true);
    setError(undefined);
    try {
      await jobs.create({ kind, durationMs, willFail });
      setItems(jobs.list());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function update(action: () => Promise<unknown>): Promise<void> {
    setBusy(true);
    setError(undefined);
    try {
      await action();
      setItems(jobs.list());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="velvet-job-operations" data-job-operations aria-label="Jobs">
      <div className="velvet-job-operations__toolbar">
        <strong>Jobs</strong>
        <VelvetButton
          data-job-create="success"
          disabled={busy}
          onClick={() => void create('Sample operation', false)}
        >
          Start task
        </VelvetButton>
        <VelvetButton
          data-job-create="failure"
          disabled={busy}
          onClick={() => void create('Sample failing operation', true)}
          variant="quiet"
        >
          Start failing task
        </VelvetButton>
        <VelvetButton
          data-job-create="long"
          disabled={busy}
          onClick={() => void create('Long-running operation', false, 30_000)}
          variant="quiet"
        >
          Start long task
        </VelvetButton>
        <span data-job-count><VelvetText as="span" tone="muted">{items.length} total</VelvetText></span>
      </div>
      {error ? <VelvetState kind="error" onRetry={() => void refresh()} title={error} /> : null}
      {items.length === 0 ? <VelvetState kind="empty" title="No jobs yet." /> : null}
      {items.length > 0 ? (
        <div className="velvet-job-operations__entries" data-job-entries>
          {[...items].reverse().map((job) => (
            <article
              className="velvet-job-operations__entry"
              data-job-entry
              data-job-id={job.id}
              data-status={job.status}
              key={job.id}
            >
              <div className="velvet-job-operations__entry-heading">
                <strong>{job.kind}</strong>
                <span data-job-status><VelvetBadge tone={job.status === 'failed' ? 'danger' : job.status === 'completed' ? 'success' : 'info'}>{statusLabel(job.status)}</VelvetBadge></span>
                <span>Attempt {job.attempt}</span>
                <span>{job.progress}%</span>
                {job.status === 'queued' || job.status === 'running' ? (
                  <VelvetButton
                    data-job-cancel
                    disabled={busy}
                    onClick={() => void update(() => jobs.cancel(job.id))}
                    variant="quiet"
                  >
                    Cancel
                  </VelvetButton>
                ) : null}
                {job.retryable && (job.status === 'failed' || job.status === 'cancelled' || job.status === 'interrupted') ? (
                  <VelvetButton
                    data-job-retry
                    disabled={busy}
                    onClick={() => void update(() => jobs.retry(job.id))}
                    variant="quiet"
                  >
                    Retry
                  </VelvetButton>
                ) : null}
              </div>
              <VelvetProgress label={`${job.kind} progress`} value={job.progress} tone={job.status === 'failed' ? 'danger' : 'accent'} />
              {job.message ? <VelvetText as="p" tone="secondary">{job.message}</VelvetText> : null}
              <div data-job-provenance>
                <span data-job-owner>Owner: {job.provenance.ownerId}</span>
                {job.provenance.assetId ? (
                  <span data-job-asset>Asset: {job.provenance.assetId}</span>
                ) : null}
                {job.provenance.providerId ? (
                  <span data-job-provider>Provider: {job.provenance.providerId}</span>
                ) : null}
              </div>
              {job.result ? (
                <details data-job-result>
                  <summary>Result · {job.result.contentType}</summary>
                  <pre>{JSON.stringify(job.result.payload, null, 2)}</pre>
                </details>
              ) : null}
            </article>
          ))}
        </div>
      ) : null}
    </section>
  );
}
