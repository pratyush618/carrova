import { useCallback, useState } from 'react';
import type { CarrovaPort } from '../../engine/types';

/** Where a text the user sent stands; `unknown`: it may have gone, check the phone. */
export type SmsSendState = 'sending' | 'no_signal' | 'failed' | 'unknown';

/** A text on its way, or one that did not (surely) go. */
export interface SmsSend {
  /** The engine sends at most once per id: a retry reuses it. */
  readonly clientId: number;
  /** Its conversation; `null` for a new message. */
  readonly thread: number | null;
  readonly to: string;
  readonly text: string;
  readonly state: SmsSendState;
}

/** A fresh id per send the user starts: 53 random bits, exact as a JS number and as the engine's `u64`. */
export function newClientId(): number {
  const [hi = 0, lo = 0] = crypto.getRandomValues(new Uint32Array(2));
  return (hi & 0x1f_ffff) * 2 ** 32 + lo;
}

/**
 * Texts sent from `device` this visit. A sent one stays until `onSent`
 * has fetched its stored copy; one that failed offers a retry under the
 * same id. Only `unknown` (or a lost answer, which may come after the
 * phone sent it) never retries: a person must not get a text twice.
 */
export function useSmsSends(
  port: Pick<CarrovaPort, 'sendSms'>,
  device: string,
  onSent: (send: SmsSend) => Promise<void>,
): {
  readonly sends: readonly SmsSend[];
  readonly send: (to: string, text: string, thread: number | null) => void;
  readonly retry: (clientId: number) => void;
} {
  const [sends, setSends] = useState<readonly SmsSend[]>([]);
  const settle = useCallback((clientId: number, state: SmsSendState | null) => {
    setSends((all) =>
      state === null
        ? all.filter((s) => s.clientId !== clientId)
        : all.map((s) => (s.clientId === clientId ? { ...s, state } : s)),
    );
  }, []);
  const run = useCallback(
    (s: SmsSend) => {
      port.sendSms(device, s.to, s.text, s.clientId).then(
        (outcome) => {
          if (outcome !== 'sent') settle(s.clientId, outcome);
          else
            void onSent(s)
              .catch(() => undefined)
              .then(() => {
                settle(s.clientId, null);
              });
        },
        () => {
          settle(s.clientId, 'unknown');
        },
      );
    },
    [port, device, onSent, settle],
  );
  const send = useCallback(
    (to: string, text: string, thread: number | null) => {
      const s: SmsSend = { clientId: newClientId(), thread, to, text, state: 'sending' };
      setSends((all) => [...all, s]);
      run(s);
    },
    [run],
  );
  const retry = useCallback(
    (clientId: number) => {
      const s = sends.find((x) => x.clientId === clientId);
      if (s?.state !== 'no_signal' && s?.state !== 'failed') return;
      settle(clientId, 'sending');
      run(s);
    },
    [sends, run, settle],
  );
  return { sends, send, retry };
}
