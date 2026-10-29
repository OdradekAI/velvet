import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import type { LayoutSnapshot, Part, WorkbenchAppId, Zone } from '../../platform/protocol.js';
import { focusTargetId, type FocusService } from '../../workbench/focus/focus-service.js';
import {
  type ViewContainerDescriptor,
  type ViewDescriptor,
  type ViewContainerScope,
  ViewRegistry,
} from '../../workbench/views/view-registry.js';
import { VelvetPanel } from '../ui/velvet-ui.js';
import { ViewRendererRegistry } from './view-renderer-registry.js';

export interface WorkbenchViewHostProps {
  readonly layout: LayoutSnapshot;
  readonly registry: ViewRegistry;
  readonly renderers?: ViewRendererRegistry;
  readonly zone: Zone;
  readonly part: Part;
  readonly scope: ViewContainerScope['kind'];
  readonly appId?: WorkbenchAppId;
  readonly focusService?: FocusService;
}

function isScopeMatch(
  scope: ViewContainerScope,
  expected: ViewContainerScope['kind'],
  appId: WorkbenchAppId | undefined,
): boolean {
  if (scope.kind !== expected) {
    return false;
  }

  return scope.kind === 'workbench' || scope.appId === appId;
}

function containerOrder(
  layout: LayoutSnapshot,
  descriptor: ViewContainerDescriptor,
): number {
  return layout.containers[descriptor.id]?.order ?? descriptor.order ?? 0;
}

function viewOrder(layout: LayoutSnapshot, descriptor: ViewDescriptor): number {
  return layout.views[descriptor.id]?.order ?? descriptor.order ?? 0;
}

function isContainerVisible(
  layout: LayoutSnapshot,
  descriptor: ViewContainerDescriptor,
): boolean {
  return layout.containers[descriptor.id]?.visible ?? descriptor.defaultVisible !== false;
}

function isViewVisible(layout: LayoutSnapshot, descriptor: ViewDescriptor): boolean {
  return layout.views[descriptor.id]?.visible ?? descriptor.defaultVisible !== false;
}

function renderFallback(view: ViewDescriptor): ReactNode {
  return (
    <p className="velvet-view-placeholder">
      {view.title} is registered and ready for its renderer.
    </p>
  );
}

export function WorkbenchViewHost({
  layout,
  registry,
  renderers,
  zone,
  part,
  scope,
  appId,
  focusService,
}: WorkbenchViewHostProps) {
  const [, rerender] = useState(0);

  useEffect(() => {
    const subscriptions = [registry.subscribe(() => rerender((value) => value + 1))];
    if (renderers) {
      subscriptions.push(renderers.subscribe(() => rerender((value) => value + 1)));
    }
    return () => subscriptions.forEach((unsubscribe) => unsubscribe());
  }, [registry, renderers]);

  const containers = registry
    .listContainers(appId)
    .filter(
      (container) =>
        container.zone === zone &&
        container.preferredPart === part &&
        isScopeMatch(container.scope, scope, appId),
    )
    .sort((left, right) => containerOrder(layout, left) - containerOrder(layout, right));

  return (
    <div className="velvet-view-host" data-velvet-view-zone={zone}>
      {containers.map((container) => {
        if (!isContainerVisible(layout, container)) {
          return null;
        }

        const views = registry
          .list(zone, appId)
          .filter(
            (view) =>
              view.containerId === container.id &&
              isScopeMatch(view.scope, scope, appId) &&
              isViewVisible(layout, view),
          )
          .sort((left, right) => viewOrder(layout, left) - viewOrder(layout, right));

        return (
          <VelvetPanel className="velvet-view-container" key={container.id} title={container.title}>
            {views.length === 0 ? (
              <p className="velvet-view-placeholder">No views are active in this container.</p>
            ) : (
              views.map((view) => {
                const renderer = renderers?.get(view.id);
                const viewContent = renderer ? renderer(view) : renderFallback(view);
                if (focusService) {
                  return (
                    <div
                      className="velvet-view"
                      data-velvet-focus-target={focusTargetId({ kind: 'view', viewId: view.id })}
                      key={view.id}
                      tabIndex={-1}
                    >
                      {viewContent}
                    </div>
                  );
                }
                return <div className="velvet-view" key={view.id}>{viewContent}</div>;
              })
            )}
          </VelvetPanel>
        );
      })}
    </div>
  );
}
