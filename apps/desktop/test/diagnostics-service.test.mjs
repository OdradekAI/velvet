import assert from 'node:assert/strict';
const { PluginDiagnosticsService } = await import(
  new URL('../src/workbench/diagnostics/diagnostics-service.ts', import.meta.url),
);

const service = new PluginDiagnosticsService();
let notifications = 0;
service.subscribe(() => {
  notifications += 1;
});
service.appendLog({
  schemaVersion: 1,
  sequence: 1,
  extensionId: 'sample.diagnostics',
  level: 'warn',
  category: 'runtime',
  message: 'bounded',
  sessionId: 'session-1',
  jobId: 'job-1',
  operationId: null,
  retryable: true,
  occurredAtMs: 1,
});
service.appendNotification({
  schemaVersion: 1,
  sequence: 1,
  extensionId: 'sample.diagnostics',
  severity: 'warning',
  title: 'Warning',
  message: 'Check the plugin',
  sessionId: 'session-1',
  jobId: null,
  operationId: null,
  retryable: true,
  occurredAtMs: 1,
});
assert.equal(service.getLogs().length, 1);
assert.equal(service.getNotifications()[0].severity, 'warning');
assert.equal(notifications, 2);
console.log('Diagnostics service tests passed.');
