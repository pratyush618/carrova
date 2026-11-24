import { useEffect, type RefObject } from 'react';

/**
 * Sets `--px` / `--py` (−1…1, the pointer's place over `ref`) for layered
 * parallax. Mouse only, and never under reduced motion; eases back to 0 on leave.
 */
export function usePointerDepth(ref: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const el = ref.current;
    const fine = window.matchMedia('(pointer: fine) and (prefers-reduced-motion: no-preference)');
    if (el === null || !fine.matches) return undefined;
    let frame = 0;
    const set = (x: number, y: number) => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        el.style.setProperty('--px', x.toFixed(3));
        el.style.setProperty('--py', y.toFixed(3));
      });
    };
    const move = (e: PointerEvent) => {
      const r = el.getBoundingClientRect();
      set(((e.clientX - r.left) / r.width) * 2 - 1, ((e.clientY - r.top) / r.height) * 2 - 1);
    };
    const leave = () => {
      set(0, 0);
    };
    el.addEventListener('pointermove', move);
    el.addEventListener('pointerleave', leave);
    return () => {
      cancelAnimationFrame(frame);
      el.removeEventListener('pointermove', move);
      el.removeEventListener('pointerleave', leave);
    };
  }, [ref]);
}
