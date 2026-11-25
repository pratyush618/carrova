import { useEffect, type RefObject } from 'react';

/**
 * Sets `--px` / `--py` (−1…1, the mouse's place over `ref`) for layered
 * parallax. Follows a mouse only, and stops (snapping back) the moment
 * reduced motion is turned on; eases back to 0 on leave.
 */
export function usePointerDepth(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = ref.current;
    if (el === null) return undefined;
    const allowed = window.matchMedia(
      '(pointer: fine) and (prefers-reduced-motion: no-preference)',
    );
    let frame = 0;
    const set = (x: number, y: number) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        el.style.setProperty('--px', x.toFixed(3));
        el.style.setProperty('--py', y.toFixed(3));
      });
    };
    const move = (e: PointerEvent) => {
      if (e.pointerType !== 'mouse' || !allowed.matches) return;
      const r = el.getBoundingClientRect();
      set(((e.clientX - r.left) / r.width) * 2 - 1, ((e.clientY - r.top) / r.height) * 2 - 1);
    };
    const leave = (e: PointerEvent) => {
      if (e.pointerType === 'mouse') set(0, 0);
    };
    // Not allowed any more: drop any queued frame and put the layers back at once.
    const changed = () => {
      if (allowed.matches) return;
      cancelAnimationFrame(frame);
      el.style.setProperty('--px', '0');
      el.style.setProperty('--py', '0');
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerleave', leave);
    allowed.addEventListener('change', changed);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerleave', leave);
      allowed.removeEventListener('change', changed);
    };
  }, [ref]);
}
