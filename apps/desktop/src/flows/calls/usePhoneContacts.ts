import { useEffect, useState } from 'react';
import type { Contact } from '../../bindings/Contact';
import type { CarrovaPort } from '../../engine/types';

/** How long typing pauses before the phone is asked. */
export const CONTACTS_DEBOUNCE_MS = 250;

const NONE: readonly Contact[] = [];

/**
 * Phone `id`'s contacts matching `query`, asked once typing pauses while
 * `on`: none for an empty query, or when the phone can't say (too old to
 * search, no contacts permission, or away).
 */
export function usePhoneContacts(
  port: Pick<CarrovaPort, 'phoneContacts'>,
  id: string,
  query: string,
  on: boolean,
): readonly Contact[] {
  const [contacts, setContacts] = useState<readonly Contact[]>([]);
  const q = query.trim();
  useEffect(() => {
    if (!on || q === '') return;
    let live = true;
    const timer = setTimeout(() => {
      port.phoneContacts(id, q).then(
        (got) => {
          if (live) setContacts(got);
        },
        () => {
          if (live) setContacts([]);
        },
      );
    }, CONTACTS_DEBOUNCE_MS);
    return () => {
      live = false;
      clearTimeout(timer);
    };
  }, [port, id, q, on]);
  return on && q !== '' ? contacts : NONE;
}
