import { createContext, useContext, useMemo, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { VelvetIconButton } from '../primitives/index.js';

export interface VelvetToastItem {
  readonly id: string;
  readonly message: string;
  readonly title?: string;
  readonly severity?: 'info' | 'success' | 'warning' | 'error';
  readonly action?: ReactNode;
}

export interface VelvetToastHostProps {
  readonly items: readonly VelvetToastItem[];
  readonly onDismiss: (id: string) => void;
  readonly className?: string;
}

export function VelvetToastHost({ items, onDismiss, className = '' }: VelvetToastHostProps) {
  return (
    <div aria-label="Notifications" className={`velvet-toast-host ${className}`.trim()} role="region">
      {items.map((item) => (
        <article
          aria-live={item.severity === 'error' ? 'assertive' : 'polite'}
          className={`velvet-toast velvet-toast--${item.severity ?? 'info'}`}
          data-toast-id={item.id}
          key={item.id}
          role={item.severity === 'error' ? 'alert' : 'status'}
        >
          <div className="velvet-toast__content">
            {item.title ? <strong>{item.title}</strong> : null}
            <span>{item.message}</span>
            {item.action}
          </div>
          <VelvetIconButton icon="×" label="Dismiss notification" onClick={() => onDismiss(item.id)} />
        </article>
      ))}
    </div>
  );
}

interface VelvetToastContextValue {
  readonly push: (item: Omit<VelvetToastItem, 'id'>) => string;
  readonly dismiss: (id: string) => void;
}

const VelvetToastContext = createContext<VelvetToastContextValue | undefined>(undefined);

export function VelvetToastProvider({ children }: { readonly children: ReactNode }) {
  const [items, setItems] = useState<readonly VelvetToastItem[]>([]);
  const nextId = useRef(0);
  const dismiss = (id: string) => setItems((current) => current.filter((item) => item.id !== id));
  const push = (item: Omit<VelvetToastItem, 'id'>): string => {
    nextId.current += 1;
    const id = `toast-${nextId.current}`;
    setItems((current) => [...current, { ...item, id }]);
    return id;
  };
  const value = useMemo(() => ({ push, dismiss }), []);
  return (
    <VelvetToastContext.Provider value={value}>
      {children}
      <VelvetToastHost items={items} onDismiss={dismiss} />
    </VelvetToastContext.Provider>
  );
}

export function useVelvetToast(): VelvetToastContextValue {
  const context = useContext(VelvetToastContext);
  if (!context) {
    throw new Error('useVelvetToast must be used inside VelvetToastProvider');
  }
  return context;
}
