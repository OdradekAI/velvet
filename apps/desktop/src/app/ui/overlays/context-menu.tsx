import * as ContextMenu from '@radix-ui/react-context-menu';
import type { ReactNode } from 'react';

export interface VelvetContextMenuAction {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly checked?: boolean;
  readonly shortcutHint?: string;
  readonly separatorBefore?: boolean;
  readonly group?: string;
  readonly onSelect?: () => void;
}

export interface VelvetContextMenuProps {
  readonly children: ReactNode;
  readonly actions: readonly VelvetContextMenuAction[];
}

export function VelvetContextMenu({ children, actions }: VelvetContextMenuProps) {
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger className="velvet-context-target">
        {children}
      </ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className="velvet-context-menu">
          {actions.map((action) => (
            <span key={action.id}>
              {action.separatorBefore ? <ContextMenu.Separator className="velvet-context-menu__separator" /> : null}
              <ContextMenu.Item
                aria-checked={action.checked}
                className="velvet-context-menu__item"
                data-checked={action.checked ? 'true' : undefined}
                disabled={action.disabled}
                onSelect={action.onSelect}
                role={action.checked === undefined ? undefined : 'menuitemcheckbox'}
              >
                <span>{action.label}</span>
                {action.shortcutHint ? <kbd>{action.shortcutHint}</kbd> : null}
              </ContextMenu.Item>
            </span>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
