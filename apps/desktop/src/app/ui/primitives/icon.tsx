import type { ReactNode } from 'react';
import type { UiCommonProps } from '../internal/types.js';
import { uiDataProps } from '../internal/ui-data-props.js';

export interface VelvetIconProps extends UiCommonProps {
  readonly name?: VelvetIconName;
  readonly children?: ReactNode;
  readonly label?: string;
  readonly className?: string;
}

export type VelvetIconName =
  | 'add'
  | 'all'
  | 'back'
  | 'check'
  | 'clock'
  | 'close'
  | 'color'
  | 'community'
  | 'filter'
  | 'format'
  | 'folder'
  | 'folder-open'
  | 'forward'
  | 'grid'
  | 'help'
  | 'image'
  | 'layout'
  | 'library'
  | 'more'
  | 'pin'
  | 'rating'
  | 'search'
  | 'settings'
  | 'shape'
  | 'shuffle'
  | 'spark'
  | 'tag'
  | 'trash'
  | 'warning'
  | 'zoom-in'
  | 'zoom-out';

export function VelvetIcon({ children, name, label, className = '', ...common }: VelvetIconProps) {
  return (
    <span
      {...uiDataProps(common)}
      aria-hidden={label ? undefined : true}
      aria-label={label}
      className={`velvet-icon ${className}`.trim()}
      data-ui-kit-component="icon"
      data-icon-name={name}
      role={label ? 'img' : undefined}
    >
      {children ?? (name ? <VelvetIconGlyph name={name} /> : null)}
    </span>
  );
}

function VelvetIconGlyph({ name }: { readonly name: VelvetIconName }) {
  const common = { fill: 'none', stroke: 'currentColor', strokeLinecap: 'round' as const, strokeLinejoin: 'round' as const, strokeWidth: 1.7 };
  switch (name) {
    case 'add': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M12 5v14M5 12h14" /></svg>;
    case 'all': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></svg>;
    case 'back': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m14 5-7 7 7 7M7 12h10" /></svg>;
    case 'check': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m5 12 4 4L19 6" /></svg>;
    case 'clock': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="12" cy="12" r="8" /><path d="M12 7v5l3 2" /></svg>;
    case 'close': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m6 6 12 12M18 6 6 18" /></svg>;
    case 'color': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="8" cy="8" r="2" /><circle cx="15" cy="7" r="2" /><circle cx="17" cy="14" r="2" /><path d="M12 20a8 8 0 1 1 8-8c0 1.1-.9 2-2 2h-2.3a2 2 0 0 0-1.8 2.8A2 2 0 0 1 12 20Z" /></svg>;
    case 'community': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="8" cy="8" r="3" /><circle cx="16" cy="9" r="2.5" /><path d="M3.5 19c.5-3 2-4.5 4.5-4.5s4 1.5 4.5 4.5M13 19c.3-2 1.4-3.3 3.4-3.3 2.1 0 3.2 1.3 3.6 3.3" /></svg>;
    case 'filter': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M4 6h16M7 12h10M10 18h4" /></svg>;
    case 'format': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><rect x="4" y="5" width="16" height="14" rx="2" /><path d="M8 9h8M8 13h5M8 16h3" /></svg>;
    case 'folder': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M3.5 7.5h6l2 2h9v8.5a2 2 0 0 1-2 2h-13a2 2 0 0 1-2-2v-8.5a2 2 0 0 1 2-2Z" /></svg>;
    case 'folder-open': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M3 8h7l2 2h9l-2 8.5a2 2 0 0 1-2 1.5H5a2 2 0 0 1-2-2L3 8Z" /><path d="M3 8V6.5a2 2 0 0 1 2-2h5l2 2h7" /></svg>;
    case 'forward': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m10 5 7 7-7 7M17 12H7" /></svg>;
    case 'grid': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><rect x="4" y="4" width="6" height="6" rx="1" /><rect x="14" y="4" width="6" height="6" rx="1" /><rect x="4" y="14" width="6" height="6" rx="1" /><rect x="14" y="14" width="6" height="6" rx="1" /></svg>;
    case 'help': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="12" cy="12" r="8.5" /><path d="M9.7 9a2.4 2.4 0 1 1 3.8 1.9c-.9.6-1.5 1-1.5 2.1M12 16.5h.01" /></svg>;
    case 'image': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><rect x="4" y="5" width="16" height="14" rx="2" /><circle cx="9" cy="10" r="1.5" /><path d="m6 17 4.5-4 3 2.5 2-1.5L20 17" /></svg>;
    case 'layout': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><rect x="4" y="5" width="16" height="14" rx="2" /><path d="M10 5v14M10 12h10" /></svg>;
    case 'library': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M5 5h11a2 2 0 0 1 2 2v12H7a2 2 0 0 1-2-2V5Z" /><path d="M7 19h12M8 8h7M8 12h7" /></svg>;
    case 'more': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="6" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="12" cy="12" r="1" fill="currentColor" stroke="none" /><circle cx="18" cy="12" r="1" fill="currentColor" stroke="none" /></svg>;
    case 'pin': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m9 4 6 6M8 5l7 7M7 8l-3 3 9 2 3 7 2-2-1-7 3-3-4-4-3 3-7-1Z" /></svg>;
    case 'rating': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m12 4 2.4 4.8 5.3.8-3.8 3.7.9 5.2-4.8-2.5-4.8 2.5.9-5.2-3.8-3.7 5.3-.8L12 4Z" /></svg>;
    case 'search': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="10.5" cy="10.5" r="5.5" /><path d="m15 15 5 5" /></svg>;
    case 'settings': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="12" cy="12" r="3" /><path d="M19 13.5a7.5 7.5 0 0 0 0-3l2-1.2-2-3.4-2.2 1a7.5 7.5 0 0 0-2.6-1.5L14 3h-4l-.3 2.4A7.5 7.5 0 0 0 7 6.9l-2.2-1-2 3.4 2 1.2a7.5 7.5 0 0 0 0 3l-2 1.2 2 3.4 2.2-1a7.5 7.5 0 0 0 2.6 1.5L10 21h4l.3-2.4a7.5 7.5 0 0 0 2.6-1.5l2.2 1 2-3.4-2.1-1.2Z" /></svg>;
    case 'shape': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="8" cy="8" r="3" /><rect x="13" y="5" width="6" height="6" rx="1" /><path d="m5 19 3-4 3 4M14 15h5l-2.5 4h-5l2.5-4Z" /></svg>;
    case 'shuffle': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M4 7h2c4 0 5 10 9 10h5M4 17h2c1.6 0 2.7-1.7 3.6-3.5M15 7h2c1.4 0 2.2 1 3 2.2M18 5l2 2-2 2M18 15l2 2-2 2" /></svg>;
    case 'spark': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m12 3 1.5 6.5L20 12l-6.5 1.5L12 20l-1.5-6.5L4 12l6.5-2.5L12 3Z" /></svg>;
    case 'tag': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M4 5v6l8 8 7-7-8-8H5a1 1 0 0 0-1 1Z" /><circle cx="8" cy="8" r="1" fill="currentColor" stroke="none" /></svg>;
    case 'trash': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="M5 7h14M10 4h4l1 3H9l1-3ZM7 7l1 13h8l1-13M10 10v7M14 10v7" /></svg>;
    case 'warning': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><path d="m12 4 9 16H3L12 4Z" /><path d="M12 9v5M12 17h.01" /></svg>;
    case 'zoom-in': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="10.5" cy="10.5" r="5.5" /><path d="m15 15 5 5M10.5 8v5M8 10.5h5" /></svg>;
    case 'zoom-out': return <svg aria-hidden="true" viewBox="0 0 24 24" {...common}><circle cx="10.5" cy="10.5" r="5.5" /><path d="m15 15 5 5M8 10.5h5" /></svg>;
  }
}
