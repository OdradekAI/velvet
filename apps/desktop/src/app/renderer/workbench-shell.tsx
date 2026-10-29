import type {
  CSSProperties,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
  ReactNode,
} from 'react';
import type {
  LayoutCommand,
  LayoutSnapshot,
  Part,
  WorkbenchAppId,
} from '../../platform/protocol.js';
import { PARTS } from '../../workbench/parts/part-registry.js';
import type { WorkbenchAppDescriptor } from '../../workbench/apps/workbench-app.js';
import { focusTargetId, type FocusService } from '../../workbench/focus/focus-service.js';
import { ViewRegistry } from '../../workbench/views/view-registry.js';
import { VelvetContextMenu, VelvetPanel } from '../ui/velvet-ui.js';
import { useFocusDomAdapter } from './focus-dom-adapter.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';
import { WorkbenchViewHost } from './workbench-view-host.js';

export interface WorkbenchShellProps {
  readonly layout: LayoutSnapshot;
  readonly apps: readonly WorkbenchAppDescriptor[];
  readonly onActivateApp: (appId: WorkbenchAppId) => void;
  readonly onLayoutCommand?: (command: LayoutCommand) => void;
  readonly focusService?: FocusService;
  readonly viewRegistry?: ViewRegistry;
  readonly viewRenderers?: ViewRendererRegistry;
  readonly children?: ReactNode;
}

function partSize(
  layout: LayoutSnapshot,
  part: Part,
  fallback: string,
): string {
  const state = layout.parts[part];
  if (!state?.visible || state.collapsed) {
    return '0px';
  }

  return state.size > 0 ? `${state.size}px` : fallback;
}

function partStyles(layout: LayoutSnapshot): CSSProperties {
  return {
    '--velvet-title-height': partSize(layout, 'title-bar', '36px'),
    '--velvet-activity-width': partSize(layout, 'activity-bar', '48px'),
    '--velvet-sidebar-width': partSize(layout, 'primary-sidebar', '260px'),
    '--velvet-auxiliary-width': partSize(layout, 'auxiliary-sidebar', '300px'),
    '--velvet-operations-height': partSize(layout, 'operations-panel', '180px'),
    '--velvet-status-height': partSize(layout, 'status-bar', '24px'),
  } as CSSProperties;
}

function isPartVisible(layout: LayoutSnapshot, part: Part): boolean {
  return layout.parts[part]?.visible ?? false;
}

function isPartCollapsed(layout: LayoutSnapshot, part: Part): boolean {
  return layout.parts[part]?.collapsed ?? false;
}

function togglePart(
  part: Part,
  onLayoutCommand: ((command: LayoutCommand) => void) | undefined,
): void {
  onLayoutCommand?.({ id: 'workbench.toggle-part', part });
}

type ResizablePart = 'primary-sidebar' | 'auxiliary-sidebar' | 'operations-panel';

function resizeBounds(part: ResizablePart): { min: number; max: number } {
  const descriptor = PARTS.find((candidate) => candidate.id === part);
  return {
    min: descriptor?.minSize ?? 0,
    max: descriptor?.maxSize ?? Number.MAX_SAFE_INTEGER,
  };
}

function clampSize(part: ResizablePart, size: number): number {
  const { min, max } = resizeBounds(part);
  return Math.min(max, Math.max(min, Math.round(size)));
}

function dispatchResize(
  part: ResizablePart,
  size: number,
  onLayoutCommand: ((command: LayoutCommand) => void) | undefined,
): void {
  onLayoutCommand?.({
    id: 'workbench.resize-part',
    part,
    size: clampSize(part, size),
  });
}

interface PartResizeHandleProps {
  readonly part: ResizablePart;
  readonly layout: LayoutSnapshot;
  readonly onLayoutCommand?: (command: LayoutCommand) => void;
  readonly placement: 'sidebar-end' | 'sidebar-start' | 'panel-start';
}

function PartResizeHandle({
  part,
  layout,
  onLayoutCommand,
  placement,
}: PartResizeHandleProps) {
  const currentSize = layout.parts[part]?.size ?? 0;

  function onPointerDown(event: ReactPointerEvent<HTMLButtonElement>): void {
    event.preventDefault();
    const startPosition = part === 'operations-panel' ? event.clientY : event.clientX;
    const direction = part === 'auxiliary-sidebar' ? -1 : 1;

    const onMove = (moveEvent: PointerEvent) => {
      const position = part === 'operations-panel' ? moveEvent.clientY : moveEvent.clientX;
      dispatchResize(
        part,
        currentSize + (position - startPosition) * direction,
        onLayoutCommand,
      );
    };
    const onEnd = () => {
      document.removeEventListener('pointermove', onMove);
      document.removeEventListener('pointerup', onEnd);
      document.removeEventListener('pointercancel', onEnd);
      document.body.removeAttribute('data-velvet-resizing');
    };

    document.body.setAttribute('data-velvet-resizing', part);
    document.addEventListener('pointermove', onMove);
    document.addEventListener('pointerup', onEnd, { once: true });
    document.addEventListener('pointercancel', onEnd, { once: true });
  }

  function onKeyDown(event: ReactKeyboardEvent<HTMLButtonElement>): void {
    const horizontalStep = event.shiftKey ? 40 : 10;
    const verticalStep = event.shiftKey ? 40 : 10;
    const step = part === 'operations-panel' ? verticalStep : horizontalStep;
    const direction = part === 'auxiliary-sidebar' ? -1 : 1;
    let delta = 0;

    if (part === 'operations-panel') {
      if (event.key === 'ArrowUp') delta = step;
      if (event.key === 'ArrowDown') delta = -step;
    } else {
      if (event.key === 'ArrowRight') delta = step * direction;
      if (event.key === 'ArrowLeft') delta = -step * direction;
    }

    if (!delta) {
      return;
    }

    event.preventDefault();
    dispatchResize(part, currentSize + delta, onLayoutCommand);
  }

  return (
    <button
      aria-label={`Resize ${part.replaceAll('-', ' ')}`}
      aria-orientation={part === 'operations-panel' ? 'horizontal' : 'vertical'}
      aria-valuemax={resizeBounds(part).max}
      aria-valuemin={resizeBounds(part).min}
      aria-valuenow={clampSize(part, currentSize)}
      className={`velvet-resize-handle velvet-resize-handle--${placement}`}
      onKeyDown={onKeyDown}
      onPointerDown={onPointerDown}
      type="button"
    />
  );
}

export function WorkbenchShell({
  layout,
  apps,
  onActivateApp,
  onLayoutCommand,
  focusService,
  viewRegistry,
  viewRenderers,
  children,
}: WorkbenchShellProps) {
  useFocusDomAdapter(focusService);
  const activeApp = apps.find(
    (workspace) => workspace.id === layout.activeApp,
  );
  const layoutActions = [
    {
      id: 'toggle-title-bar',
      label: `${isPartVisible(layout, 'title-bar') ? 'Hide' : 'Show'} Title Bar`,
      onSelect: () => togglePart('title-bar', onLayoutCommand),
    },
    {
      id: 'toggle-activity-bar',
      label: `${isPartVisible(layout, 'activity-bar') ? 'Hide' : 'Show'} Activity Bar`,
      onSelect: () => togglePart('activity-bar', onLayoutCommand),
    },
    {
      id: 'toggle-primary-sidebar',
      label: `${isPartVisible(layout, 'primary-sidebar') ? 'Hide' : 'Show'} Primary Sidebar`,
      onSelect: () => togglePart('primary-sidebar', onLayoutCommand),
    },
    {
      id: 'toggle-auxiliary-sidebar',
      label: `${isPartVisible(layout, 'auxiliary-sidebar') ? 'Hide' : 'Show'} Auxiliary Sidebar`,
      onSelect: () => togglePart('auxiliary-sidebar', onLayoutCommand),
    },
    {
      id: 'toggle-operations-panel',
      label: `${isPartVisible(layout, 'operations-panel') ? 'Hide' : 'Show'} Operations Panel`,
      onSelect: () => togglePart('operations-panel', onLayoutCommand),
    },
    {
      id: 'toggle-status-bar',
      label: `${isPartVisible(layout, 'status-bar') ? 'Hide' : 'Show'} Status Bar`,
      onSelect: () => togglePart('status-bar', onLayoutCommand),
    },
    {
      id: 'collapse-primary-sidebar',
      label: `${isPartCollapsed(layout, 'primary-sidebar') ? 'Expand' : 'Collapse'} Primary Sidebar`,
      onSelect: () =>
        onLayoutCommand?.({
          id: 'workbench.collapse-part',
          part: 'primary-sidebar',
          collapsed: !isPartCollapsed(layout, 'primary-sidebar'),
        }),
    },
    {
      id: 'collapse-auxiliary-sidebar',
      label: `${isPartCollapsed(layout, 'auxiliary-sidebar') ? 'Expand' : 'Collapse'} Auxiliary Sidebar`,
      onSelect: () =>
        onLayoutCommand?.({
          id: 'workbench.collapse-part',
          part: 'auxiliary-sidebar',
          collapsed: !isPartCollapsed(layout, 'auxiliary-sidebar'),
        }),
    },
    {
      id: 'collapse-operations-panel',
      label: `${isPartCollapsed(layout, 'operations-panel') ? 'Expand' : 'Collapse'} Operations Panel`,
      onSelect: () =>
        onLayoutCommand?.({
          id: 'workbench.collapse-part',
          part: 'operations-panel',
          collapsed: !isPartCollapsed(layout, 'operations-panel'),
        }),
    },
    {
      id: 'reset-layout',
      label: 'Reset Workbench Layout',
      onSelect: () => onLayoutCommand?.({ id: 'workbench.reset-layout' }),
    },
  ];

  return (
    <div className="velvet-app">
      <main className="velvet-workbench" style={partStyles(layout)}>
        <header
          className="velvet-part velvet-part--title"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'title-bar' })}
          hidden={!isPartVisible(layout, 'title-bar')}
          tabIndex={-1}
        >
          <span>Velvet / {activeApp?.title ?? 'Workbench'}</span>
          <span className="velvet-title__hint">Right-click workspace for layout</span>
        </header>

        <nav
          aria-label="Workspaces"
          className="velvet-part velvet-part--activity"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'activity-bar' })}
          hidden={!isPartVisible(layout, 'activity-bar')}
          tabIndex={-1}
        >
          {apps.map((app) => (
            <button
              aria-label={app.title}
              className="velvet-workspace-button"
              data-active={app.id === layout.activeApp}
              key={app.id}
              onClick={() => onActivateApp(app.id)}
              title={app.title}
              type="button"
            >
              {app.activityBarIcon.slice(0, 2).toUpperCase()}
            </button>
          ))}
        </nav>

        <aside
          className="velvet-part velvet-part--primary"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'primary-sidebar' })}
          hidden={
            !isPartVisible(layout, 'primary-sidebar') ||
            isPartCollapsed(layout, 'primary-sidebar')
          }
          tabIndex={-1}
        >
          <VelvetPanel title="Navigation">
            {viewRegistry ? (
              <WorkbenchViewHost
                appId={layout.activeApp}
                focusService={focusService}
                layout={layout}
                part="primary-sidebar"
                registry={viewRegistry}
                renderers={viewRenderers}
                scope="workbench-app"
                zone="navigation"
              />
            ) : (
              <>
                <p>{activeApp?.title ?? 'Workbench'}</p>
                <p>Views registered in the navigation zone appear here.</p>
              </>
            )}
          </VelvetPanel>
          <PartResizeHandle
            layout={layout}
            onLayoutCommand={onLayoutCommand}
            part="primary-sidebar"
            placement="sidebar-end"
          />
        </aside>

        <section
          className="velvet-part velvet-part--workspace"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'workspace-area' })}
          tabIndex={-1}
        >
          <VelvetContextMenu
            actions={[
              {
                id: 'open-preview',
                label: 'Open preview',
                onSelect: () => undefined,
              },
              {
                id: 'inspect-asset',
                label: 'Inspect asset',
                onSelect: () => undefined,
              },
              ...layoutActions,
            ]}
          >
            <div className="velvet-workspace-content">
              {children ?? <div className="velvet-empty-state">Workspace area</div>}
              {viewRegistry ? (
                <WorkbenchViewHost
                  appId={layout.activeApp}
                  focusService={focusService}
                  layout={layout}
                  part="workspace-area"
                  registry={viewRegistry}
                  renderers={viewRenderers}
                  scope="workbench-app"
                  zone="contextual"
                />
              ) : null}
              {viewRegistry ? (
                <WorkbenchViewHost
                  appId={layout.activeApp}
                  focusService={focusService}
                  layout={layout}
                  part="workspace-area"
                  registry={viewRegistry}
                  renderers={viewRenderers}
                  scope="workbench-app"
                  zone="operations"
                />
              ) : null}
            </div>
          </VelvetContextMenu>
        </section>

        <aside
          className="velvet-part velvet-part--auxiliary"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'auxiliary-sidebar' })}
          hidden={
            !isPartVisible(layout, 'auxiliary-sidebar') ||
            isPartCollapsed(layout, 'auxiliary-sidebar')
          }
          tabIndex={-1}
        >
          <VelvetPanel title="Auxiliary">
            {viewRegistry ? (
              <WorkbenchViewHost
                focusService={focusService}
                layout={layout}
                part="auxiliary-sidebar"
                registry={viewRegistry}
                renderers={viewRenderers}
                scope="workbench"
                zone="contextual"
              />
            ) : <p>Inspectors and preview augmentations appear here.</p>}
          </VelvetPanel>
          <PartResizeHandle
            layout={layout}
            onLayoutCommand={onLayoutCommand}
            part="auxiliary-sidebar"
            placement="sidebar-start"
          />
        </aside>

        <section
          className="velvet-part velvet-part--operations"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'operations-panel' })}
          hidden={
            !isPartVisible(layout, 'operations-panel') ||
            isPartCollapsed(layout, 'operations-panel')
          }
          tabIndex={-1}
        >
          <VelvetPanel title="Operations">
            {viewRegistry ? (
              <WorkbenchViewHost
                focusService={focusService}
                layout={layout}
                part="operations-panel"
                registry={viewRegistry}
                renderers={viewRenderers}
                scope="workbench"
                zone="operations"
              />
            ) : <p>Jobs, logs and operational tools appear here.</p>}
          </VelvetPanel>
          <PartResizeHandle
            layout={layout}
            onLayoutCommand={onLayoutCommand}
            part="operations-panel"
            placement="panel-start"
          />
        </section>

        <footer
          className="velvet-part velvet-part--status"
          data-velvet-focus-target={focusTargetId({ kind: 'part', part: 'status-bar' })}
          hidden={!isPartVisible(layout, 'status-bar')}
          tabIndex={-1}
        >
          Ready · Layout schema {layout.schemaVersion}
        </footer>
      </main>
    </div>
  );
}
