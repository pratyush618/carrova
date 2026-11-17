import { useEffect, useState } from 'react';
import type { CarrovaPort } from '../../engine/types';

/**
 * The name of linked device `id`, or with `null` of the first linked
 * phone (`''` until known, or when there is none).
 */
export function useLinkedName(port: CarrovaPort, id: string | null): string {
  const [name, setName] = useState('');
  useEffect(() => {
    let live = true;
    port.linkedDevices().then(
      (devices) => {
        const device = devices.find((d) => (id === null ? d.os === 'android' : d.id === id));
        if (live) setName(device?.name ?? '');
      },
      () => undefined,
    );
    return () => {
      live = false;
    };
  }, [port, id]);
  return name;
}
