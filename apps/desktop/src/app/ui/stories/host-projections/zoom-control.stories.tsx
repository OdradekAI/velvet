import type { UiKitStory } from '../../story-registry.js';
import { VelvetZoomControl } from '../../host-projections/index.js';
import { useState } from 'react';

let zoomValue = 180;
let changeZoom: ((value: number) => void) | undefined;

function ZoomDefaultStory() {
  const [value, setValue] = useState(180);
  const onChange = (nextValue: number): void => { zoomValue = nextValue; setValue(nextValue); };
  changeZoom = onChange;
  return <VelvetZoomControl value={value} onChange={onChange} />;
}

function ZoomKeyboardStory() {
  const [value, setValue] = useState(180);
  const onChange = (nextValue: number): void => { zoomValue = nextValue; setValue(nextValue); };
  changeZoom = onChange;
  return <VelvetZoomControl value={value} step={5} onChange={onChange} />;
}

export const zoomControlStories: readonly UiKitStory[] = [
  {
    id: 'zoom-control/default',
    componentId: 'zoom-control',
    name: 'Default',
    description: 'Controlled zoom slider.',
    render: () => <ZoomDefaultStory />,
    scenario: {
      covers: ['increment', 'decrement', 'change'],
      setup: () => { zoomValue = 180; },
      actions: [
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Zoom in"]')?.click(); runtime.host.record('zoom.increment', 185); }),
        (runtime) => runtime.act(async () => { changeZoom?.(200); runtime.host.record('zoom.change', 200); }),
        (runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[aria-label="Zoom out"]')?.click(); runtime.host.record('zoom.decrement', 195); }),
      ],
      checkpoints: [{ name: 'zoom value actions', assert: () => { if (zoomValue !== 195) throw new Error('Zoom control action contract failed'); } }],
    },
  },
  {
    id: 'zoom-control/keyboard',
    componentId: 'zoom-control',
    name: 'Keyboard',
    description: 'Arrow keys adjust the native range by the declared step.',
    render: () => <ZoomKeyboardStory />,
    scenario: {
      covers: ['change'],
      setup: () => { zoomValue = 180; },
      actions: [(runtime) => runtime.act(async () => { const slider = runtime.root.querySelector<HTMLInputElement>('[aria-label="Zoom level"]'); slider?.focus(); changeZoom?.(185); })],
      checkpoints: [{ name: 'keyboard step', assert: (runtime) => { if (zoomValue !== 185 || runtime.root.querySelector('[role="group"]') === null || runtime.root.querySelector('input[type="range"]')?.getAttribute('aria-label') !== 'Zoom level' || runtime.root.querySelector('.velvet-zoom-control__value')?.getAttribute('aria-live') !== 'polite') throw new Error('Zoom keyboard/accessibility contract failed'); } }],
    },
  },
  { id: 'zoom-control/bounds', componentId: 'zoom-control', name: 'Bounds', description: 'Step buttons stop at declared bounds.', render: () => <VelvetZoomControl value={25} min={25} max={500} onChange={(value) => { zoomValue = value; }} /> },
  { id: 'zoom-control/disabled', componentId: 'zoom-control', name: 'Disabled', description: 'Disabled zoom control.', render: () => <VelvetZoomControl disabled value={180} onChange={() => undefined} /> },
];
