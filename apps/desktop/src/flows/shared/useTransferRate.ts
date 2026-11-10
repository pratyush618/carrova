import { useEffect, useRef, useState } from 'react';
import { addSample, rateOf, type RateSample } from './transferView';

/** How often the rate is re-measured while bytes may flow. */
const SAMPLE_MS = 1_000;

/**
 * Bytes per second from how `done` grew over the last seconds, sampled
 * while `flowing` (a stall drops the rate to unknown once the window passes).
 */
export function useTransferRate(done: number, flowing: boolean): number | null {
  const latest = useRef(done);
  const [samples, setSamples] = useState<readonly RateSample[]>([]);
  useEffect(() => {
    latest.current = done;
  }, [done]);
  useEffect(() => {
    if (!flowing) return undefined;
    const id = setInterval(() => {
      setSamples((prev) => addSample(prev, { at: Date.now(), done: latest.current }));
    }, SAMPLE_MS);
    return () => {
      clearInterval(id);
    };
  }, [flowing]);
  return flowing ? rateOf(samples) : null;
}
