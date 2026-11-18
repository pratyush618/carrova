import { useEffect, useState } from 'react';
import type { Feature } from '../../bindings/Feature';
import type { SimChoice } from '../../bindings/SimChoice';
import type { CarrovaPort } from '../../engine/types';

/**
 * The SIMs phone `id` offers to text (`messages`) or call (`calls`) from,
 * asked while `on`: none with one SIM, or when the phone can't say (too
 * old to list them, or away), so no picker shows and it uses its default.
 * `null` until the phone answers.
 */
export function usePhoneSims(
  port: Pick<CarrovaPort, 'phoneSims'>,
  id: string,
  feature: Feature,
  on: boolean,
): readonly SimChoice[] | null {
  // Tagged with what was asked, so another phone's answer never shows.
  const key = `${id} ${feature}`;
  const [got, setGot] = useState<{ readonly key: string; readonly sims: readonly SimChoice[] }>();
  useEffect(() => {
    if (!on) return;
    let live = true;
    port.phoneSims(id, feature).then(
      (sims) => {
        if (live) setGot({ key, sims });
      },
      () => {
        if (live) setGot({ key, sims: [] });
      },
    );
    return () => {
      live = false;
    };
  }, [port, id, feature, on, key]);
  if (!on) return [];
  return got?.key === key ? got.sims : null;
}

/** The SIM to preselect: `preferred` (a thread's last) while it's offered, else the phone's default; `null` with no choice. */
export const presetSim = (sims: readonly SimChoice[], preferred: number | null): number | null =>
  (sims.find((s) => s.id === preferred) ?? sims.find((s) => s.default) ?? sims[0])?.id ?? null;
