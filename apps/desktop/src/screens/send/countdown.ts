import { useEffect, useState } from 'react';

/** Whole seconds from `now` until `deadline` (both ms since epoch); never below 0. */
export const secondsLeft = (deadline: number, now: number): number =>
  Math.max(0, Math.ceil((deadline - now) / 1000));

/** `292` → `4:52`. */
export const clock = (seconds: number): string =>
  `${String(Math.floor(seconds / 60))}:${String(seconds % 60).padStart(2, '0')}`;

/**
 * What a screen reader hears: whole minutes left, rounded up, and `0` for
 * "under a minute". It changes once a minute, so a live region holding it
 * doesn't announce every tick.
 */
export const minutesLeft = (seconds: number): number =>
  seconds > 60 ? Math.ceil(seconds / 60) : 0;

/** Seconds left until `deadline` (ms since epoch), re-read every second. */
export function useSecondsLeft(deadline: number): number {
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const id = window.setInterval(() => {
      setNow(Date.now());
    }, 1000);
    return () => {
      window.clearInterval(id);
    };
  }, []);
  return secondsLeft(deadline, now);
}
