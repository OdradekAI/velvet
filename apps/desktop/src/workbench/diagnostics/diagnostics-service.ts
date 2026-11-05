import type {
  CorePluginDiagnosticEvent,
  CorePluginLogRecord,
  CorePluginNotificationRecord,
} from '../../platform/protocol.js';

const MAX_RECORDS = 256;

export class PluginDiagnosticsService {
  private readonly diagnostics: CorePluginDiagnosticEvent[] = [];
  private readonly logs: CorePluginLogRecord[] = [];
  private readonly notifications: CorePluginNotificationRecord[] = [];
  private readonly listeners = new Set<() => void>();

  public subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  }

  public appendDiagnostic(record: CorePluginDiagnosticEvent): void {
    appendBounded(this.diagnostics, record);
    this.notify();
  }

  public appendLog(record: CorePluginLogRecord): void {
    appendBounded(this.logs, record);
    this.notify();
  }

  public appendNotification(record: CorePluginNotificationRecord): void {
    appendBounded(this.notifications, record);
    this.notify();
  }

  public replaceLogs(records: readonly CorePluginLogRecord[]): void {
    replaceBounded(this.logs, records);
    this.notify();
  }

  public replaceNotifications(records: readonly CorePluginNotificationRecord[]): void {
    replaceBounded(this.notifications, records);
    this.notify();
  }

  public getDiagnostics(): readonly CorePluginDiagnosticEvent[] {
    return [...this.diagnostics];
  }

  public getLogs(): readonly CorePluginLogRecord[] {
    return [...this.logs];
  }

  public getNotifications(): readonly CorePluginNotificationRecord[] {
    return [...this.notifications];
  }

  private notify(): void {
    for (const listener of this.listeners) {
      try {
        listener();
      } catch {
        // Diagnostics must not block workbench rendering or transport recovery.
      }
    }
  }
}

function appendBounded<T>(target: T[], record: T): void {
  target.push(record);
  if (target.length > MAX_RECORDS) {
    target.splice(0, target.length - MAX_RECORDS);
  }
}

function replaceBounded<T>(target: T[], records: readonly T[]): void {
  target.splice(0, target.length, ...records.slice(-MAX_RECORDS));
}
