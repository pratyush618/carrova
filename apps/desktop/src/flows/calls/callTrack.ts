import type { Caller } from '../../bindings/Caller';
import type { CallView } from '../../bindings/CallView';

/** A linked phone's call as this computer follows it (`call_view` events). */
export interface CallTrack {
  /** What to show now: `none` once the call is over (a missed call stays until dismissed). */
  readonly view: CallView;
  /** Who is on it, from its ringing; `null` for a call made from the phone. */
  readonly caller: Caller | null;
  /** The phone that left mid-call ("<Phone> left"), until the next call. */
  readonly left: string | null;
}

export const noCall: CallTrack = { view: { kind: 'none' }, caller: null, left: null };

/**
 * How long a `left` waits while the phone rings: a reconnect sends `left`
 * then `ringing` again, and the window must not flicker nor the ringtone
 * restart.
 */
export const LEFT_GRACE_MS = 3000;

/** `view` (the engine's latest) folded into `track`. */
export function foldCall(track: CallTrack, view: CallView): CallTrack {
  switch (view.kind) {
    case 'ringing':
    case 'missed':
      return { view, caller: { number: view.number, name: view.name }, left: null };
    case 'active': {
      // Only an answered (or running) call keeps its caller; one made from the phone has none.
      const answered = track.view.kind === 'ringing' || track.view.kind === 'active';
      return { view, caller: answered ? track.caller : null, left: null };
    }
    case 'left':
      return { ...noCall, left: view.device };
    case 'none':
      // The phone goes idle after a missed call: the note stays until dismissed.
      return track.view.kind === 'missed' ? track : noCall;
  }
}

/** Whether `view` is a `left` to hold back for {@link LEFT_GRACE_MS}: its phone is ringing. */
export const holdsLeft = (track: CallTrack, view: CallView): boolean =>
  view.kind === 'left' && track.view.kind === 'ringing' && track.view.device === view.device;

/** What to call `caller` (`null`: nobody known). */
export const callerName = (caller: Caller | null): string | null =>
  caller === null ? null : (caller.name ?? caller.number);
