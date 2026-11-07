import type { VelvetTabsProps } from '../../primitives/tabs.js';
import { defineComponentContract, type ComponentContract } from '../../internal/component-contract.js';

export const VelvetTabsContract = defineComponentContract<
  VelvetTabsProps,
  'selected' | 'disabled' | 'focus',
  'select' | 'navigate',
  'tab-selection' | 'focus' | 'keyboard'
>({
  schema: 'velvet.ui.component-contract',
  schemaVersion: 1,
  id: 'tabs',
  layer: 'primitives',
  owner: 'host',
  props: ['tabs', 'selectedId', 'onSelect'],
  slots: ['tab labels'],
  states: ['selected', 'disabled', 'focus'],
  actions: ['select', 'navigate'],
  observables: ['tab-selection', 'focus', 'keyboard'],
  stories: ['default', 'disabled'],
  documentation: 'docs/engineering/ui-kit/components/primitives/tabs.md',
  evidence: [
    { kind: 'host-guarantee', reference: 'docs/engineering/ui-kit-component-guide.md', claim: 'Velvet semantic component contract and accessibility rules' },
    { kind: 'observed', reference: 'apps/desktop/test/ui-kit-components.test.mjs', claim: 'Executable renderer behavior coverage' },
  ],
} satisfies ComponentContract<VelvetTabsProps, 'selected' | 'disabled' | 'focus', 'select' | 'navigate', 'tab-selection' | 'focus' | 'keyboard'>);
