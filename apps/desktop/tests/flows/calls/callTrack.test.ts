import { describe, expect, it } from 'vitest';
import type { CallView } from '@/bindings/CallView';
import { foldCall, holdsLeft, noCall } from '@/flows/calls/callTrack';

const ringing: CallView = {
  kind: 'ringing',
  device: 'p8',
  number: '+15550101',
  name: 'Sam Rivera',
  ring_here: true,
};
const active: CallView = {
  kind: 'active',
  device: 'p8',
  since_ms: 0,
  muted: false,
  audio: 'phone',
  outgoing: false,
};

describe('foldCall', () => {
  it('keeps the caller from the ringing once the call is answered', () => {
    const track = foldCall(foldCall(noCall, ringing), active);
    expect(track.view).toEqual(active);
    expect(track.caller).toEqual({ number: '+15550101', name: 'Sam Rivera' });
  });

  it('closes on left and names the phone that left', () => {
    const track = foldCall(foldCall(noCall, active), { kind: 'left', device: 'p8' });
    expect(track.view.kind).toBe('none');
    expect(track.left).toBe('p8');
  });

  it('keeps a missed call shown when the phone goes idle after it', () => {
    const missed: CallView = { kind: 'missed', device: 'p8', number: '+1', name: null };
    const track = foldCall(foldCall(noCall, missed), { kind: 'none' });
    expect(track.view).toEqual(missed);
  });

  it('holds a left back only while that phone rings', () => {
    const left: CallView = { kind: 'left', device: 'p8' };
    expect(holdsLeft(foldCall(noCall, ringing), left)).toBe(true);
    expect(holdsLeft(foldCall(noCall, active), left)).toBe(false);
    expect(holdsLeft(foldCall(noCall, ringing), { kind: 'left', device: 'd2' })).toBe(false);
  });
});
