import * as Dialog from '@radix-ui/react-dialog';
import { useEffect, useRef } from 'react';
import type { ReactNode, RefObject } from 'react';
import { VelvetButton } from '../primitives/index.js';
import type { UiButtonVariant } from '../internal/types.js';

export interface VelvetDialogProps {
  readonly open: boolean;
  readonly onOpenChange: (open: boolean) => void;
  readonly title: string;
  readonly description?: string;
  readonly children: ReactNode;
  readonly actions?: readonly VelvetDialogAction[];
  readonly severity?: 'normal' | 'warning' | 'danger';
  readonly closeLabel?: string;
  readonly restoreFocusRef?: RefObject<HTMLElement | null>;
}

export interface VelvetDialogAction {
  readonly id: string;
  readonly label: string;
  readonly variant?: UiButtonVariant;
  readonly primary?: boolean;
  readonly disabled?: boolean;
  readonly busy?: boolean;
  readonly onSelect: () => void;
}

export function VelvetDialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  actions = [],
  severity = 'normal',
  closeLabel = 'Close',
  restoreFocusRef,
}: VelvetDialogProps) {
  const previousOpenRef = useRef(open);
  useEffect(() => {
    if (previousOpenRef.current && !open) {
      queueMicrotask(() => restoreFocusRef?.current?.focus());
    }
    previousOpenRef.current = open;
  }, [open, restoreFocusRef]);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="velvet-dialog__overlay" />
        <Dialog.Content className={`velvet-dialog__content velvet-dialog__content--${severity}`}>
          <Dialog.Title className="velvet-dialog__title">{title}</Dialog.Title>
          {description ? (
            <Dialog.Description className="velvet-dialog__description">
              {description}
            </Dialog.Description>
          ) : null}
          <div className="velvet-dialog__body">{children}</div>
          <div className="velvet-dialog__actions">
            {actions.map((action) => (
              <VelvetButton
                busy={action.busy}
                className={action.primary ? 'velvet-dialog__action--primary' : ''}
                disabled={action.disabled}
                key={action.id}
                onClick={action.onSelect}
                variant={action.variant ?? (action.primary ? 'default' : 'quiet')}
              >
                {action.label}
              </VelvetButton>
            ))}
            <Dialog.Close className="velvet-button velvet-button--quiet">
              {closeLabel}
            </Dialog.Close>
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
