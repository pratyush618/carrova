import { useEffect, useState } from 'react';
import type { Feature } from '../../bindings/Feature';
import type { SimChoice } from '../../bindings/SimChoice';
import type { CarrovaPort } from '../../engine/types';

/**
 * The SIMs phone `id` offers to text (`messages`) or call (`calls`) from,
 * asked while `on`: none with one SIM, or when the phone can't say (too
 * old to list them, or away), so no picker shows and it uses its default.
 */
export function usePhoneSims(
  port: Pick<CarrovaPort, 'phoneSims'>,
  id: string,
  feature: Feature,
  on: boolean,
): readonly SimChoice[] {
  const [sims, setSims] = useState<readonly SimChoice[]>([]);
  useEffect(() => {
    if (!on) return;
    let live = true;
    port.phoneSims(id, feature).then(
      (got) => {
        if (live) setSims(got);
      },
      () => {
        if (live) setSims([]);
      },
    );
    return () => {
      live = false;
    };
  }, [port, id, feature, on]);
  return on ? sims : [];
}

/** The SIM to preselect: `preferred` (a thread's last) while it's offered, else the phone's default; `null` with no choice. */
export const presetSim = (sims: readonly SimChoice[], preferred: number | null): number | null =>
  (sims.find((s) => s.id === preferred) ?? sims.find((s) => s.default) ?? sims[0])?.id ?? null;
