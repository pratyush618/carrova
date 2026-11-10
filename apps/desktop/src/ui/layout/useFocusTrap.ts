import { useEffect, useEffectEvent, type RefObject } from 'react';

const FOCUSABLE = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  'summary',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function focusables(root: HTMLElement): HTMLElement[] {
  return [...root.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((el) => !el.closest('[inert]'));
}

/**
 * Modal focus handling: moves focus in (to `[data-initial-focus]`, else the
 * heading), keeps Tab inside, calls `onEscape` on Esc, and gives focus back
 * to whatever had it when the overlay closes.
 */
export function useFocusTrap(ref: RefObject<HTMLElement | null>, onEscape: () => void): void {
  const escape = useEffectEvent(onEscape);

  useEffect(() => {
    const root = ref.current;
    if (!root) return undefined;
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    (
      root.querySelector<HTMLElement>('[data-initial-focus]') ??
      root.querySelector<HTMLElement>('h2')
    )?.focus();

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        escape();
        return;
      }
      if (event.key !== 'Tab') return;
      const items = focusables(root);
      const first = items[0];
      const last = items.at(-1);
      if (!first || !last) {
        event.preventDefault();
        return;
      }
      const active = document.activeElement;
      if (
        event.shiftKey &&
        (active === first || !root.contains(active) || active === root.querySelector('h2'))
      ) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && active === last) {
        event.preventDefault();
        first.focus();
      }
    };

    root.addEventListener('keydown', onKeyDown);
    return () => {
      root.removeEventListener('keydown', onKeyDown);
      previous?.focus();
    };
  }, [ref]);
}
