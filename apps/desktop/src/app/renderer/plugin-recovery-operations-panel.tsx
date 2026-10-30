import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  CorePluginInstallRecoveryAction,
  CorePluginInstallRecoveryLogEntry,
  CorePluginInstallRecoveryLogStatus,
} from '../../platform/protocol.js';
import type { PluginClient } from '../../workbench/plugins/plugin-client.js';

export interface PluginRecoveryOperationsPanelProps {
  readonly plugins: PluginClient;
  readonly exportLogs?: () => Promise<string | undefined>;
}

type LogStatusFilter = 'all' | CorePluginInstallRecoveryLogStatus;
type LogActionFilter = 'all' | CorePluginInstallRecoveryAction;

function formatAction(action: CorePluginInstallRecoveryAction): string {
  return action[0].toUpperCase() + action.slice(1);
}

function formatTime(timestamp: number): string {
  if (!timestamp) {
    return 'Unknown time';
  }
  return new Date(timestamp).toLocaleString();
}

function entryMatches(
  entry: CorePluginInstallRecoveryLogEntry,
  search: string,
  status: LogStatusFilter,
  action: LogActionFilter,
): boolean {
  if (status !== 'all' && entry.status !== status) {
    return false;
  }
  if (action !== 'all' && entry.action !== action) {
    return false;
  }
  if (!search) {
    return true;
  }
  const haystack = [
    entry.extensionId,
    entry.action,
    entry.status,
    entry.message,
    ...entry.removedPaths,
  ].join(' ').toLowerCase();
  return haystack.includes(search);
}

/** Presents host-owned plugin recovery history alongside global operations. */
export function PluginRecoveryOperationsPanel({
  plugins,
  exportLogs,
}: PluginRecoveryOperationsPanelProps) {
  const [logs, setLogs] = useState<readonly CorePluginInstallRecoveryLogEntry[]>([]);
  const [search, setSearch] = useState('');
  const [status, setStatus] = useState<LogStatusFilter>('all');
  const [action, setAction] = useState<LogActionFilter>('all');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string>();
  const [error, setError] = useState<string>();

  const refresh = useCallback(async () => {
    try {
      setError(undefined);
      setLogs(await plugins.queryCoreInstallRecoveryLog());
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setLoading(false);
    }
  }, [plugins]);

  useEffect(() => {
    void refresh();
    return plugins.subscribe(() => {
      void refresh();
    });
  }, [plugins, refresh]);

  const visibleLogs = useMemo(() => [...logs]
    .sort((left, right) => right.occurredAtMs - left.occurredAtMs)
    .filter((entry) => entryMatches(entry, search.trim().toLowerCase(), status, action)),
  [action, logs, search, status]);

  async function clearLog(): Promise<void> {
    setBusy(true);
    setError(undefined);
    setMessage(undefined);
    try {
      await plugins.clearCoreInstallRecoveryLog();
      setLogs([]);
      setMessage('Recovery log cleared.');
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  async function exportLog(): Promise<void> {
    if (!exportLogs) {
      setError('Native export is unavailable in this host.');
      return;
    }
    setBusy(true);
    setError(undefined);
    setMessage(undefined);
    try {
      const destination = await exportLogs();
      if (destination) {
        setMessage(`Recovery log exported to ${destination}.`);
      }
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="velvet-plugin-recovery-operations" data-plugin-recovery-operations>
      <div className="velvet-plugin-recovery-operations__toolbar">
        <input
          aria-label="Filter recovery logs"
          className="velvet-input velvet-plugin-recovery-operations__search"
          data-plugin-recovery-log-search
          onChange={(event) => setSearch(event.target.value)}
          placeholder="Filter plugin, message or path"
          value={search}
        />
        <select
          aria-label="Filter recovery log status"
          className="velvet-select"
          data-plugin-recovery-log-status
          onChange={(event) => setStatus(event.target.value as LogStatusFilter)}
          value={status}
        >
          <option value="all">All status</option>
          <option value="failed">Failed</option>
          <option value="succeeded">Succeeded</option>
        </select>
        <select
          aria-label="Filter recovery log action"
          className="velvet-select"
          data-plugin-recovery-log-action
          onChange={(event) => setAction(event.target.value as LogActionFilter)}
          value={action}
        >
          <option value="all">All actions</option>
          <option value="resume">Resume</option>
          <option value="commit">Commit</option>
          <option value="rollback">Rollback</option>
          <option value="discard">Discard</option>
        </select>
        <button
          className="velvet-button velvet-button--quiet"
          data-plugin-recovery-log-export
          disabled={busy || logs.length === 0}
          onClick={() => void exportLog()}
          type="button"
        >
          Export
        </button>
        <button
          className="velvet-button velvet-button--quiet"
          data-plugin-recovery-log-clear
          disabled={busy || logs.length === 0}
          onClick={() => void clearLog()}
          type="button"
        >
          Clear
        </button>
      </div>
      {message ? <p className="velvet-plugin-recovery-operations__message" data-plugin-recovery-log-message>{message}</p> : null}
      {error ? <p className="velvet-plugin-recovery-operations__error" data-plugin-recovery-log-error>{error}</p> : null}
      {loading ? <p className="velvet-muted">Loading recovery log…</p> : null}
      {!loading && visibleLogs.length === 0 ? (
        <p className="velvet-muted" data-plugin-recovery-log-empty>
          {logs.length === 0 ? 'No plugin recovery log entries.' : 'No entries match these filters.'}
        </p>
      ) : null}
      {visibleLogs.length > 0 ? (
        <div className="velvet-plugin-recovery-operations__entries" data-plugin-recovery-log-entries>
          {visibleLogs.map((entry, index) => (
            <article
              className="velvet-plugin-recovery-operations__entry"
              data-plugin-recovery-log-entry
              data-status={entry.status}
              key={`${entry.extensionId}-${entry.action}-${entry.attempt}-${entry.occurredAtMs}-${index}`}
            >
              <div className="velvet-plugin-recovery-operations__entry-heading">
                <strong>{entry.extensionId}</strong>
                <span data-plugin-recovery-log-status>{entry.status}</span>
                <span>{formatAction(entry.action)} · attempt {entry.attempt}</span>
                <time dateTime={new Date(entry.occurredAtMs).toISOString()}>{formatTime(entry.occurredAtMs)}</time>
              </div>
              <p>{entry.message}</p>
              {entry.removedPaths.length > 0 ? (
                <small>Removed: {entry.removedPaths.join(', ')}</small>
              ) : null}
            </article>
          ))}
        </div>
      ) : null}
    </div>
  );
}
