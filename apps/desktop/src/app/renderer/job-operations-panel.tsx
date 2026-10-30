import { useCallback, useEffect, useState } from 'react';
import type { CoreJobSnapshot } from '../../platform/protocol.js';
import type { JobClient } from '../../workbench/jobs/job-client.js';

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
        <button
          className="velvet-button"
          data-job-create="success"
          disabled={busy}
          onClick={() => void create('Sample operation', false)}
          type="button"
        >
          Start task
        </button>
        <button
          className="velvet-button velvet-button--quiet"
          data-job-create="failure"
          disabled={busy}
          onClick={() => void create('Sample failing operation', true)}
          type="button"
        >
          Start failing task
        </button>
        <button
          className="velvet-button velvet-button--quiet"
          data-job-create="long"
          disabled={busy}
          onClick={() => void create('Long-running operation', false, 30_000)}
          type="button"
        >
          Start long task
        </button>
        <span className="velvet-muted" data-job-count>{items.length} total</span>
      </div>
      {error ? <p className="velvet-plugin-recovery-operations__error" data-job-error>{error}</p> : null}
      {items.length === 0 ? <p className="velvet-muted" data-job-empty>No jobs yet.</p> : null}
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
                <span data-job-status>{statusLabel(job.status)}</span>
                <span>Attempt {job.attempt}</span>
                <span>{job.progress}%</span>
                {job.status === 'queued' || job.status === 'running' ? (
                  <button
                    className="velvet-button velvet-button--quiet"
                    data-job-cancel
                    disabled={busy}
                    onClick={() => void update(() => jobs.cancel(job.id))}
                    type="button"
                  >
                    Cancel
                  </button>
                ) : null}
                {job.retryable && (job.status === 'failed' || job.status === 'cancelled' || job.status === 'interrupted') ? (
                  <button
                    className="velvet-button velvet-button--quiet"
                    data-job-retry
                    disabled={busy}
                    onClick={() => void update(() => jobs.retry(job.id))}
                    type="button"
                  >
                    Retry
                  </button>
                ) : null}
              </div>
              <progress max={100} value={job.progress} />
              {job.message ? <p>{job.message}</p> : null}
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
