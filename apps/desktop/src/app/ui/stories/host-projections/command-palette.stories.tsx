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
    description: 'Host-owned query text is reported while the result projection remains host-owned.',
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
        if (input?.value !== 'Pin' || runtime.root.querySelector('[data-command-results="host"]') === null) throw new Error('CommandPalette controlled query contract failed');
      } }],
    },
  },
  {
    id: 'command-palette/host-results',
    componentId: 'command-palette',
    name: 'Host results',
    description: 'The host updates the result list after receiving a query change.',
    render: () => <HostResultsStory />,
    scenario: {
      covers: ['query'],
      actions: [(runtime) => runtime.act(async () => {
        const input = runtime.root.querySelector<HTMLInputElement>('[role="dialog"] input');
        if (!input) throw new Error('CommandPalette host result input missing');
        Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), 'value')?.set?.call(input, 'Pin');
        input.dispatchEvent(new Event('input', { bubbles: true }));
      })],
      checkpoints: [{ name: 'host result projection', afterAction: 0, assert: (runtime) => {
        if (runtime.root.querySelector('[role="option"]')?.textContent?.trim() !== 'Pin selected' || runtime.root.querySelector('[data-command-results="host"]') === null) throw new Error('CommandPalette host results contract failed');
      } }],
    },
  },
];

function CommandScopeStory() {
  const [scope, setScope] = useState<'all' | 'current'>('all');
  const commands = scope === 'current' ? [{ id: 'current', label: 'Open current library', scope: 'current' as const }] : [{ id: 'all', label: 'Open all libraries', scope: 'all' as const }, { id: 'current', label: 'Open current library', scope: 'current' as const }];
  return <VelvetCommandPalette commands={commands} onClose={noop} onScopeChange={setScope} onSelect={noop} open scope={scope} scopeOptions={[{ id: 'all', label: 'All' }, { id: 'current', label: 'Current' }]} />;
}

function ControlledQueryStory() {
  const [query, setQuery] = useState('');
  const [commands, setCommands] = useState([{ id: 'open', label: 'Open selected' }, { id: 'pin', label: 'Pin selected' }]);
  const onQueryChange = (nextQuery: string) => {
    setQuery(nextQuery);
    setCommands(nextQuery.toLowerCase().includes('pin') ? [{ id: 'pin', label: 'Pin selected' }] : [{ id: 'open', label: 'Open selected' }, { id: 'pin', label: 'Pin selected' }]);
  };
  return <VelvetCommandPalette
    commands={commands}
    onClose={noop}
    onQueryChange={onQueryChange}
    onSelect={noop}
    open
    query={query}
  />;
}

function HostResultsStory() {
  const [query, setQuery] = useState('');
  const [commands, setCommands] = useState([{ id: 'open', label: 'Open selected' }, { id: 'pin', label: 'Pin selected' }]);
  return <VelvetCommandPalette
    commands={commands}
    onClose={noop}
    onQueryChange={(nextQuery) => { setQuery(nextQuery); setCommands(nextQuery === 'Pin' ? [{ id: 'pin', label: 'Pin selected' }] : []); }}
    onSelect={noop}
    open
    query={query}
  />;
}
