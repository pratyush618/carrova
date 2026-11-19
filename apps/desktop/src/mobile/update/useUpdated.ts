import { useCallback, useEffect, useState } from 'react';
import type { CarrovaPort } from '../../engine/types';

/** The version this phone was updated to since it last ran (asked once), until dismissed. */
export function useUpdated(port: CarrovaPort): {
  readonly version: string | null;
  readonly dismiss: () => void;
} {
  const [version, setVersion] = useState<string | null>(null);
  useEffect(() => {
    port.takeUpdated().then(setVersion, () => undefined);
  }, [port]);
  const dismiss = useCallback(() => {
    setVersion(null);
  }, []);
  return { version, dismiss };
}
