import type { Found, Invite } from '../../app/state/EngineState';
import type { LinkedDeviceView, PairInviteView, PairPeerView } from '../../engine/types';
import type { AddDeviceModel } from '../../screens/devices/model';

/** How a pairing attempt ended. */
export type PairEnd =
  | { readonly state: 'linked'; readonly device: LinkedDeviceView | null }
  | { readonly state: 'expired' | 'failed' };

/** An attempt's invite and when it stops working (ms since epoch). */
export interface AttemptInvite {
  readonly view: PairInviteView;
  readonly expiresAt: number;
}

/** The invite of the attempt started at `attempt`; one from an earlier attempt is stale. */
export function attemptInvite(invite: Invite | null, attempt: number): AttemptInvite | null {
  if (invite === null || invite.receivedAt < attempt) return null;
  return { view: invite.view, expiresAt: invite.receivedAt + invite.view.expires_in_s * 1000 };
}

/** The device found for the attempt started at `attempt`; one from an earlier attempt is stale. */
export function attemptPeer(found: Found | null, attempt: number): PairPeerView | null {
  return found === null || found.receivedAt < attempt ? null : found.view;
}

/**
 * The end after `next` arrives: a link always wins; an expired code stays
 * expired when the engine then gives up on it (that is no connection failure).
 */
export function settle(end: PairEnd | null, next: PairEnd): PairEnd {
  if (end?.state === 'linked' || (end?.state === 'expired' && next.state === 'failed')) return end;
  return next;
}

/**
 * What the Add a device dialog shows, `secondsLeft` on the attempt's code.
 * A device found waits for the user's answer even once the code runs out;
 * `answered` once the user chose, while the engine finishes.
 */
export function addDeviceModel(
  current: AttemptInvite | null,
  found: PairPeerView | null,
  end: PairEnd | null,
  secondsLeft: number,
  answered = false,
): AddDeviceModel {
  if (end?.state === 'linked') return end;
  if (end === null && found !== null) return { state: 'found', peer: found, answered };
  if (end?.state === 'expired' || (current !== null && secondsLeft === 0))
    return { state: 'expired' };
  if (end !== null) return end;
  return current === null
    ? { state: 'starting' }
    : { state: 'waiting', invite: current.view, expiresAt: current.expiresAt };
}
