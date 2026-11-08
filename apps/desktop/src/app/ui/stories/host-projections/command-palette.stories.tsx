import type { UiKitStory } from '../../story-registry.js';
import { useState } from 'react';
import { VelvetCommandPalette } from '../../host-projections/index.js';

let selectedCommand = '';
let closed = false;
const noop = () => undefined;

export const commandPaletteStories: readonly UiKitStory[] = [
  {
    id: 'command-palette/open',
    componentId: 'command-palette',
    name: 'Open',
    description: 'Open command palette.',
    render: () => <VelvetCommandPalette open commands={[{ id: 'open', label: 'Open' }, { id: 'pin', label: 'Pin' }]} onSelect={(id) => { selectedCommand = id; }} onClose={() => { closed = true; }} />,
    scenario: {
      covers: ['select', 'navigate', 'close'],
      setup: () => { selectedCommand = ''; closed = false; },
      actions: [
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.focus();
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'ArrowDown' }));
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Home' }));
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'End' }));
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Enter' }));
          runtime.host.record('command.select', 'pin');
        }),
        (runtime) => runtime.act(async () => {
          const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
          input?.dispatchEvent(new KeyboardEvent('keydown', { bubbles: true, key: 'Escape' }));
        }),
      ],
      checkpoints: [
        { name: 'keyboard navigation', afterAction: 0, assert: (runtime) => { if (runtime.root.querySelector('[role="option"][aria-selected="true"]')?.textContent?.trim() !== 'Pin') throw new Error('CommandPalette ArrowDown contract failed'); } },
        { name: 'home/end navigation', afterAction: 1, assert: (runtime) => { if (runtime.root.querySelector('[role="option"][aria-selected="true"]')?.textContent?.trim() !== 'Pin') throw new Error('CommandPalette Home/End contract failed'); } },
        { name: 'keyboard command selection', afterAction: 2, assert: () => { if (selectedCommand !== 'pin') throw new Error('CommandPalette keyboard selection contract failed'); } },
        { name: 'escape close', afterAction: 3, assert: () => { if (!closed) throw new Error('CommandPalette Escape close contract failed'); } },
      ],
    },
  },
  { id: 'command-palette/empty', componentId: 'command-palette', name: 'Empty', description: 'Empty command result.', render: () => <VelvetCommandPalette open commands={[]} onSelect={noop} onClose={noop} /> },
  {
    id: 'command-palette/scope',
    componentId: 'command-palette',
    name: 'Scope tabs',
    description: 'Host-controlled current/all search scope with active descendant semantics.',
    render: () => <CommandScopeStory />,
    scenario: {
      covers: ['scope-change', 'query'],
      actions: [(runtime) => runtime.act(async () => { runtime.root.querySelector<HTMLButtonElement>('[role="tab"][aria-label="Current"]')?.click(); })],
      checkpoints: [{ name: 'scope and active descendant', afterAction: 0, assert: (runtime) => {
        if (runtime.root.querySelector('[role="tab"][aria-selected="true"]')?.textContent !== 'Current' || runtime.root.querySelector('[role="option"]') === null || runtime.root.querySelector('input')?.getAttribute('aria-activedescendant') === null) throw new Error('CommandPalette scope/active descendant contract failed');
      } }],
    },
  },
  {
    id: 'command-palette/controlled-query',
    componentId: 'command-palette',
    name: 'Controlled query',
    description: 'Host-owned query text keeps filtering and input observation in one owner.',
    render: () => <ControlledQueryStory />,
    scenario: {
      covers: ['query'],
      setup: () => { selectedCommand = ''; },
      actions: [(runtime) => runtime.act(async () => {
        const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
        if (!input) throw new Error('CommandPalette controlled query input missing');
        Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), 'value')?.set?.call(input, 'Pin');
        input.dispatchEvent(new Event('input', { bubbles: true }));
      })],
      checkpoints: [{ name: 'host query owner', afterAction: 0, assert: (runtime) => {
        const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
        if (input?.value !== 'Pin' || runtime.root.querySelectorAll('[role="option"]').length !== 1 || runtime.root.querySelector('[role="option"]')?.textContent?.trim() !== 'Pin selected') throw new Error('CommandPalette controlled query contract failed');
      } }],
    },
  },
  {
    id: 'command-palette/boolean-query',
    componentId: 'command-palette',
    name: 'Boolean query',
    description: 'Quoted terms, OR groups and negation use deterministic host search semantics.',
    render: () => <VelvetCommandPalette
      commands={[{ id: 'blue-sky', label: 'Blue sky' }, { id: 'blue-ocean', label: 'Blue ocean' }, { id: 'red-sky', label: 'Red sky' }, { id: 'green-field', label: 'Green field' }]}
      onClose={noop}
      onQueryChange={noop}
      onSelect={noop}
      open
      query={'"Blue sky" || ocean -red'}
    />,
    scenario: {
      covers: ['query'],
      actions: [],
      checkpoints: [{ name: 'boolean query semantics', assert: (runtime) => {
        const results = [...runtime.root.querySelectorAll('[role="option"]')].map((node) => node.textContent?.trim());
        if (runtime.root.querySelector('[data-command-query-semantics="boolean"]') === null || results.length !== 2 || !results.some((result) => result === 'Blue sky') || !results.some((result) => result === 'Blue ocean')) throw new Error('CommandPalette boolean query contract failed');
      } }],
    },
  },
];

function CommandScopeStory() {
  const [scope, setScope] = useState<'all' | 'current'>('all');
  return <VelvetCommandPalette commands={[{ id: 'all', label: 'Open all libraries', scope: 'all' }, { id: 'current', label: 'Open current library', scope: 'current' }]} onClose={noop} onScopeChange={setScope} onSelect={noop} open scope={scope} scopeOptions={[{ id: 'all', label: 'All' }, { id: 'current', label: 'Current' }]} />;
}

function ControlledQueryStory() {
  const [query, setQuery] = useState('');
  return <VelvetCommandPalette
    commands={[{ id: 'open', label: 'Open selected' }, { id: 'pin', label: 'Pin selected' }]}
    onClose={noop}
    onQueryChange={setQuery}
    onSelect={noop}
    open
    query={query}
  />;
}
