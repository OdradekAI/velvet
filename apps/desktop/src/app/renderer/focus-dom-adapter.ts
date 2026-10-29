import { useEffect } from 'react';
import {
  FocusService,
  type FocusTarget,
  focusTargetId,
  parseFocusTargetId,
} from '../../workbench/focus/focus-service.js';

type FocusRoot = Document | HTMLElement;

/** Bridges framework-neutral host focus state to real DOM focusable regions. */
export class FocusDomAdapter {
  private readonly unsubscribe: () => void;
  private readonly onFocusInBound = (event: Event) => this.onFocusIn(event);
  private applyingTargetId: string | undefined;
  private readonly observer?: MutationObserver;
  private resolvedTargetId: string | undefined;

  public constructor(
    private readonly service: FocusService,
    private readonly root: FocusRoot = document,
  ) {
    this.root.addEventListener('focusin', this.onFocusInBound);
    this.unsubscribe = service.subscribe((snapshot) => {
      if (snapshot.current) {
        this.focusTarget(snapshot.current);
      }
    });

    if (typeof MutationObserver !== 'undefined') {
      this.observer = new MutationObserver(() => this.reconcileCurrentTarget());
      this.observer.observe(this.root, {
        attributes: true,
        attributeFilter: ['aria-hidden', 'data-velvet-focus-target', 'hidden'],
        childList: true,
        subtree: true,
      });
    }

    const current = service.snapshot().current;
    if (current) {
      this.focusTarget(current);
    }
  }

  public dispose(): void {
    this.unsubscribe();
    this.observer?.disconnect();
    this.root.removeEventListener('focusin', this.onFocusInBound);
    this.applyingTargetId = undefined;
    this.resolvedTargetId = undefined;
  }

  private onFocusIn(event: Event): void {
    const element = event.target instanceof HTMLElement
      ? event.target.closest<HTMLElement>('[data-velvet-focus-target]')
      : null;
    const targetId = element?.dataset.velvetFocusTarget;
    if (!targetId) {
      return;
    }

    if (targetId === this.applyingTargetId) {
      this.applyingTargetId = undefined;
      return;
    }

    const target = parseFocusTargetId(targetId);
    if (target) {
      this.service.focus(target);
    }
  }

  private focusTarget(target: FocusTarget): void {
    const targetId = focusTargetId(target);
    const element = this.findTarget(targetId);
    if (!element) {
      return;
    }

    this.resolvedTargetId = targetId;
    const activeElement = this.root instanceof Document
      ? this.root.activeElement
      : this.root.ownerDocument?.activeElement;
    if (element === activeElement) {
      return;
    }

    this.applyingTargetId = targetId;
    element.focus({ preventScroll: true });
  }

  private findTarget(targetId: string): HTMLElement | undefined {
    const element = Array.from(
      this.root.querySelectorAll<HTMLElement>('[data-velvet-focus-target]'),
    ).find((element) => element.dataset.velvetFocusTarget === targetId);
    if (!element || element.hidden || element.closest('[hidden], [aria-hidden="true"]')) {
      return undefined;
    }
    return element;
  }

  private reconcileCurrentTarget(): void {
    const current = this.service.snapshot().current;
    if (!current) {
      return;
    }

    const currentId = focusTargetId(current);
    if (this.findTarget(currentId)) {
      this.resolvedTargetId = currentId;
      return;
    }

    if (this.resolvedTargetId === currentId) {
      this.resolvedTargetId = undefined;
      this.service.restore();
    }
  }
}

/** React lifecycle adapter for the host FocusService. */
export function useFocusDomAdapter(service: FocusService | undefined): void {
  useEffect(() => {
    if (!service) {
      return undefined;
    }

    const adapter = new FocusDomAdapter(service);
    return () => adapter.dispose();
  }, [service]);
}
