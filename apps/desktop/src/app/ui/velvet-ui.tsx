import * as ContextMenu from '@radix-ui/react-context-menu';
import * as Dialog from '@radix-ui/react-dialog';
import type { ReactNode } from 'react';

export interface VelvetPanelProps {
  readonly title?: string;
  readonly children: ReactNode;
  readonly className?: string;
  readonly ariaLabel?: string;
  readonly id?: string;
}

export function VelvetPanel({
  title,
  children,
  className = '',
  ariaLabel,
  id,
}: VelvetPanelProps) {
  return (
    <section aria-label={ariaLabel} className={`velvet-panel ${className}`} id={id}>
      {title ? <header className="velvet-panel__header">{title}</header> : null}
      <div className="velvet-panel__body">{children}</div>
    </section>
  );
}

export interface VelvetDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly title: string;
  readonly description?: string;
  readonly children: ReactNode;
}

export function VelvetDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
}: VelvetDialogProps) {
  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="velvet-dialog__overlay" />
        <Dialog.Content className="velvet-dialog__content">
          <Dialog.Title className="velvet-dialog__title">{title}</Dialog.Title>
          {description ? (
            <Dialog.Description className="velvet-dialog__description">
              {description}
            </Dialog.Description>
          ) : null}
          <div className="velvet-dialog__body">{children}</div>
          <Dialog.Close className="velvet-button velvet-button--quiet">
            Close
          </Dialog.Close>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export interface VelvetContextMenuAction {
  readonly id: string;
  readonly label: string;
  readonly disabled?: boolean;
  readonly onSelect: () => void;
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
            <ContextMenu.Item
              className="velvet-context-menu__item"
              disabled={action.disabled}
              key={action.id}
              onSelect={action.onSelect}
            >
              {action.label}
            </ContextMenu.Item>
          ))}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
