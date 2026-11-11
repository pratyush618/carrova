import { describe, expect, it } from 'vitest';
import type { View } from '@/app/nav/navMachine';
import { forwardOf, type MoveStates } from '@/app/shell/forward';
import { initialSaveFlow } from '@/flows/archive/saveFlow';
import { initialReceiveFlow } from '@/flows/receive/receiveFlow';
import { initialSendFlow } from '@/flows/send/sendFlow';

const HOME: View = { kind: 'hub', page: 'home' };
const SEND: View = { kind: 'flow', at: { flow: 'send', step: 'found' } };
const idle: MoveStates = {
  send: initialSendFlow,
  save: initialSaveFlow(''),
  receive: initialReceiveFlow,
};
const loading = { status: 'loading', last: null } as const;

describe('forwarded sends', () => {
  it('start a Manual send from anywhere while nothing moves', () => {
    expect(forwardOf(idle, HOME)).toBe('start');
    expect(forwardOf(idle, SEND)).toBe('start');
  });

  it('add to the selection already open on Send', () => {
    const open = { ...idle, send: { ...initialSendFlow, session: 's-1' } };
    expect(forwardOf(open, SEND)).toBe('add');
    expect(forwardOf(open, HOME)).toBe('start');
    expect(forwardOf({ ...open, send: { ...open.send, step: 'done' } }, SEND)).toBe('start');
  });

  it('forwarded_paths_refused_while_sending, saving or receiving', () => {
    expect(forwardOf({ ...idle, send: { ...initialSendFlow, run: loading } }, SEND)).toBe('refuse');
    expect(forwardOf({ ...idle, save: { ...idle.save, save: loading } }, HOME)).toBe('refuse');
    const receiving = (receive: Partial<MoveStates['receive']>) =>
      forwardOf({ ...idle, receive: { ...initialReceiveFlow, ...receive } }, HOME);
    expect(receiving({ code: { ...initialReceiveFlow.code, status: 'connecting' } })).toBe(
      'refuse',
    );
    expect(receiving({ file: { ...initialReceiveFlow.file, status: 'opening' } })).toBe('refuse');
    expect(receiving({ session: 'r-1' })).toBe('refuse');
    expect(receiving({ session: 'r-1', apply: loading })).toBe('refuse');
    expect(receiving({ session: 'r-1', apply: { status: 'ready', value: {} as never } })).toBe(
      'start',
    );
  });
});
