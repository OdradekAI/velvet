import type { VelvetZoomControlProps } from '../../host-projections/zoom-control.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetZoomControlContract = defineComponentContract<
  VelvetZoomControlProps,
  'default' | 'disabled' | 'min' | 'max' | 'keyboard',
  'increment' | 'decrement' | 'change',
  'group-role' | 'slider-role' | 'value' | 'focus' | 'disabled'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'zoom-control',
  layer: 'host-projections',
  qualification: 'both',
  owner: 'host',
  props: ['value', 'min', 'max', 'step', 'onChange', 'disabled'],
  slots: [],
  states: ['default', 'disabled', 'min', 'max', 'keyboard'],
  actions: ['increment', 'decrement', 'change'],
  browserActions: ['increment', 'decrement', 'change'],
  observables: ['group-role', 'slider-role', 'value', 'focus', 'disabled'],
  stories: ['default', 'keyboard', 'bounds', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/host-projections/zoom-control.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'The host owns durable zoom state and the component remains controlled.' },
    { kind: 'observed', reference: 'apps/desktop/tools/qualify-ui-host-projections.mjs', claim: 'Browser qualification for keyboard step, value and live output.' },
  ],
} satisfies ComponentContract<
  VelvetZoomControlProps,
  'default' | 'disabled' | 'min' | 'max' | 'keyboard',
  'increment' | 'decrement' | 'change',
  'group-role' | 'slider-role' | 'value' | 'focus' | 'disabled'
>);
