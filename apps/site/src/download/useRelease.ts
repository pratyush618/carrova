import { useEffect, useState } from 'react';
import type { ReleaseAsset } from './platform';

export interface Release {
  readonly version: string;
  readonly assets: readonly ReleaseAsset[];
}

const API = 'https://api.github.com/repos/pratyush618/carrova/releases/latest';

/** Latest release from GitHub. Until it loads (or if it fails) links fall back to the releases page. */
export function useRelease(): Release | undefined {
  const [release, setRelease] = useState<Release>();
  useEffect(() => {
    const ctrl = new AbortController();
    fetch(API, { signal: ctrl.signal })
      .then((r) =>
        r.ok ? (r.json() as Promise<{ tag_name: string; assets: ReleaseAsset[] }>) : undefined,
      )
      .then((j) => {
        if (j) setRelease({ version: j.tag_name, assets: j.assets });
      })
      .catch(() => undefined);
    return () => {
      ctrl.abort();
    };
  }, []);
  return release;
}
