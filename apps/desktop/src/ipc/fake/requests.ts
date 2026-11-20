/**
 * Move requests arriving at this computer. In `move-request`, Desk PC asks
 * a few steps after the first subscriber; Review opens the fake's import
 * session, as a code-paired receive does.
 */
import type { MoveRequestView } from '../../bindings/MoveRequestView';
import type { SessionOpened } from '../../bindings/SessionOpened';
import { REQUEST_SECS } from '../../screens/send/countdown';
import type { CarrovaPort } from '../port';
import { bridgeError, raise } from '../reject';
import { later, type Runtime } from './runtime';
import type { Scenario } from './scenario';

/** Pacing steps between the first subscriber and the request. */
const ARRIVE_STEPS = 4;

/** Desk PC's request, expiring `REQUEST_SECS` after `now` (Unix seconds). */
const deskPc = (now: number): MoveRequestView => ({
  id: 1,
  device: 'd2',
  name: 'Desk PC',
  summary: { apps: 12, bytes: 340_000_000, has_sensitive: true },
  expires_at: now + REQUEST_SECS,
});

export interface FakeRequests extends Pick<CarrovaPort, 'answerRequest'> {
  /** A listener subscribed: in `move-request`, the request is on its way (once). */
  readonly subscribed: () => void;
}

/**
 * `open` starts the import session a Review opens; `notify` puts the bell's
 * notice up for a request that arrived.
 */
export function fakeRequests(
  runtime: Runtime,
  scenario: Scenario,
  open: () => Promise<SessionOpened>,
  notify: (request: MoveRequestView) => void,
): FakeRequests {
  let pending: MoveRequestView | null = null;
  let asked = scenario !== 'move-request';
  // shortcut: an unanswered request never expires here; emit `move_request_ended` if a test needs it.
  const arrive = async () => {
    for (let step = 0; step < ARRIVE_STEPS; step++) await runtime.pause();
    pending = deskPc(Math.floor(Date.now() / 1000));
    runtime.emit({ kind: 'move_request_arrived', request: pending });
    notify(pending);
  };
  return {
    subscribed: () => {
      if (asked) return;
      asked = true;
      void arrive();
    },
    answerRequest: (id, answer) =>
      later(() => {
        if (pending?.id !== id) raise(bridgeError('session', 'that request has ended'));
        pending = null;
      }).then(() => (answer === 'review' ? open() : null)),
  };
}
