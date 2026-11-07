import type { ReactNode } from 'react';
import { VelvetButton, type VelvetButtonProps } from './button.js';

export interface VelvetIconButtonProps
  extends Omit<VelvetButtonProps, 'children' | 'label' | 'ariaLabel'> {
  readonly icon: ReactNode;
  readonly label: string;
}

export function VelvetIconButton({ icon, label, className = '', ...props }: VelvetIconButtonProps) {
  return (
    <VelvetButton
      {...props}
      ariaLabel={label}
      className={`velvet-icon-button ${className}`.trim()}
    >
      <span aria-hidden="true">{icon}</span>
    </VelvetButton>
  );
}
