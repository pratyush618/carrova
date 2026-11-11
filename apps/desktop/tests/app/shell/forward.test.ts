import { describe, expect, it, vi } from 'vitest';
import type { View } from '@/app/nav/navMachine';
import { forwarder, forwardOf, type ForwardTarget, type MoveStates } from '@/app/shell/forward';
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

describe('forwarded sends while a Manual send starts', () => {
  const starting = { ...initialSendFlow, mode: 'manual', scan: loading } as const;

  it('queue paths while the Manual session is still opening', () => {
    expect(forwardOf({ ...idle, send: starting }, SEND)).toBe('queue');
    expect(forwardOf({ ...idle, send: { ...starting, mode: 'auto' } }, SEND)).toBe('start');
  });
});

/** A bridge whose `takeSendRequest` answers each call when the test says so. */
function bridge() {
  const answers: ((paths: string[]) => void)[] = [];
  const take = vi.fn(
    () =>
      new Promise<string[]>((resolve) => {
        answers.push(resolve);
      }),
  );
  return { take, answer: (i: number, paths: string[]) => answers[i]?.(paths) };
}

function target(states: MoveStates, view: View) {
  const send = { start: vi.fn(), addPaths: vi.fn() };
  const go = vi.fn();
  const now: { current: ForwardTarget } = { current: { states, view, send, go } };
  return { send, go, now };
}

const flush = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('the forwarder', () => {
  it('forwarded_drains_run_one_after_another', async () => {
    const { take, answer } = bridge();
    const { send, now } = target({ ...idle, send: { ...initialSendFlow, session: 's-1' } }, SEND);
    const refused = vi.fn();
    const forward = forwarder(take, () => now.current, refused);
    void forward.drain();
    void forward.drain();
    await flush();
    expect(take).toHaveBeenCalledTimes(1);
    // The first request joins the open selection; the second arrives once it is sent.
    answer(0, ['/a']);
    await flush();
    expect(send.addPaths).toHaveBeenCalledWith(['/a']);
    expect(take).toHaveBeenCalledTimes(2);
    now.current = {
      ...now.current,
      states: { ...idle, send: { ...initialSendFlow, run: loading } },
    };
    answer(1, ['/b']);
    await flush();
    expect(refused.mock.calls).toEqual([[false], [true]]);
  });

  it('forwarded_paths_join_the_manual_send_that_is_starting', async () => {
    const { take, answer } = bridge();
    const { send, go, now } = target(idle, HOME);
    const forward = forwarder(take, () => now.current, vi.fn());
    void forward.drain();
    await flush();
    answer(0, ['/a']);
    await flush();
    expect(send.start).toHaveBeenCalledWith('manual', ['/a']);
    expect(go).toHaveBeenCalledOnce();
    // The second request lands before the app re-renders the starting send.
    void forward.drain();
    await flush();
    answer(1, ['/b']);
    await flush();
    forward.settle(null, 'loading');
    expect(send.start).toHaveBeenCalledOnce();
    expect(send.addPaths).not.toHaveBeenCalled();
    forward.settle('s-1', 'ready');
    expect(send.addPaths).toHaveBeenCalledWith(['/b']);
    forward.settle('s-1', 'ready');
    expect(send.addPaths).toHaveBeenCalledOnce();
  });

  it('drops queued paths when the Manual send fails to start', async () => {
    const { take, answer } = bridge();
    const { send, now } = target(
      { ...idle, send: { ...initialSendFlow, mode: 'manual', scan: loading } },
      SEND,
    );
    const forward = forwarder(take, () => now.current, vi.fn());
    void forward.drain();
    await flush();
    answer(0, ['/b']);
    await flush();
    forward.settle(null, 'failed');
    forward.settle('s-2', 'ready');
    expect(send.addPaths).not.toHaveBeenCalled();
    expect(send.start).not.toHaveBeenCalled();
  });

  it('starts a new send after the opening one was closed', async () => {
    const { take, answer } = bridge();
    const { send, now } = target(idle, HOME);
    const forward = forwarder(take, () => now.current, vi.fn());
    void forward.drain();
    await flush();
    answer(0, ['/a']);
    await flush();
    forward.settle(null, 'loading');
    // Closing Send while it opens resets the send to idle.
    forward.settle(null, 'idle');
    void forward.drain();
    await flush();
    answer(1, ['/b']);
    await flush();
    expect(send.start).toHaveBeenLastCalledWith('manual', ['/b']);
  });
});
