import type { VelvetStackProps } from './stack.js';
import { VelvetStack } from './stack.js';

export interface VelvetInlineProps extends Omit<VelvetStackProps, 'direction'> {}

export function VelvetInline(props: VelvetInlineProps) {
  return <VelvetStack {...props} direction="row" />;
}
