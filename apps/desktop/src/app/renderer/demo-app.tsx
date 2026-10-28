import { useState } from 'react';
import type { LayoutCommand, LayoutSnapshot, WorkbenchAppId } from '../../platform/protocol.js';
import { defaultLayoutState } from '../../workbench/layout/layout-state.js';
import { FocusService } from '../../workbench/focus/focus-service.js';
import { BUILTIN_WORKBENCH_APPS } from '../../workspaces/index.js';
import { WorkbenchShell } from './workbench-shell.js';
import { VelvetDialog } from '../ui/velvet-ui.js';

function applyDemoLayoutCommand(
  current: LayoutSnapshot,
  command: LayoutCommand,
): LayoutSnapshot {
  const revision = current.revision + 1;

  switch (command.id) {
    case 'workbench.activate-app':
      return { ...current, activeApp: command.appId, revision };
    case 'workbench.set-part-visibility':
      return {
        ...current,
        revision,
        parts: {
          ...current.parts,
          [command.part]: { ...current.parts[command.part], visible: command.visible },
        },
      };
    case 'workbench.toggle-part':
      return {
        ...current,
        revision,
        parts: {
          ...current.parts,
          [command.part]: {
            ...current.parts[command.part],
            visible: !current.parts[command.part].visible,
          },
        },
      };
    case 'workbench.collapse-part':
      return {
        ...current,
        revision,
        parts: {
          ...current.parts,
          [command.part]: { ...current.parts[command.part], collapsed: command.collapsed },
        },
      };
    case 'workbench.resize-part':
      return {
        ...current,
        revision,
        parts: {
          ...current.parts,
          [command.part]: { ...current.parts[command.part], size: command.size },
        },
      };
    case 'workbench.upsert-view-container':
      return {
        ...current,
        revision,
        containers: {
          ...current.containers,
          [command.containerId]: { visible: command.visible, order: command.order },
        },
      };
    case 'workbench.set-view-container-visibility':
      return {
        ...current,
        revision,
        containers: {
          ...current.containers,
          [command.containerId]: {
            ...current.containers[command.containerId],
            visible: command.visible,
          },
        },
      };
    case 'workbench.remove-view-container': {
      const { [command.containerId]: _removed, ...containers } = current.containers;
      return { ...current, revision, containers };
    }
    case 'workbench.reset-layout':
      return { ...defaultLayoutState(), revision };
    case 'workbench.focus-part':
    case 'workbench.maximize-part':
    case 'workbench.move-view':
    case 'workbench.reorder-view-container':
    case 'workbench.reset-app-layout':
    case 'workbench.reset-view-layout':
      return { ...current, revision };
  }
}

export function DemoApp() {
  const [layout, setLayout] = useState<LayoutSnapshot>(() => defaultLayoutState());
  const [focusService] = useState(() => new FocusService());
  const [dialogOpen, setDialogOpen] = useState(false);

  function dispatchLayout(command: LayoutCommand) {
    setLayout((current) => applyDemoLayoutCommand(current, command));
  }

  function activateApp(appId: WorkbenchAppId) {
    dispatchLayout({ id: 'workbench.activate-app', appId });
  }

  return (
    <>
      <WorkbenchShell
        layout={layout}
        onActivateApp={activateApp}
        onLayoutCommand={dispatchLayout}
        focusService={focusService}
        apps={BUILTIN_WORKBENCH_APPS}
      >
        <div className="velvet-empty-state">
          <div>
            <p>Workspace area</p>
            <button
              className="velvet-button"
              onClick={() => setDialogOpen(true)}
              type="button"
            >
              Open dialog
            </button>
          </div>
        </div>
      </WorkbenchShell>
      <VelvetDialog
        description="The overlay is host-owned and can be opened by a view command."
        onOpenChange={setDialogOpen}
        open={dialogOpen}
        title="Velvet dialog"
      >
        This first slice demonstrates the UI seam. Persisted layout state still
        comes from the Rust LayoutService in the production adapter.
      </VelvetDialog>
    </>
  );
}
