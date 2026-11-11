import { act, renderHook, waitFor } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { MoveDefaults } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';
import { receiveProgressModel } from '@/flows/receive/progressViews';
import { useReceiveFlow } from '@/flows/receive/useReceiveFlow';
import { emptySession } from '@/app/state/appState';

const DEFAULTS: MoveDefaults = {
  conflict_rule: 'replace',
  retention: 'until_dismissed',
  include_caches: false,
};
const LOST = { code: 'network', message: 'the sender went away', hint: null } as const;

/** A LAN receive whose apply failed with the sender gone: the progress screen offers Resume. */
async function lostTransfer() {
  const port = createFakePort({ scenario: 'happy', stepMs: 0 });
  vi.spyOn(port, 'approve').mockResolvedValue();
  vi.spyOn(port, 'apply').mockRejectedValue(LOST);
  const flow = renderHook(() => useReceiveFlow(port, {}, DEFAULTS));
  act(() => {
    flow.result.current.start('lan');
    flow.result.current.setCode('7-acid-lemon');
  });
  act(() => {
    flow.result.current.open();
  });
  await waitFor(() => {
    expect(flow.result.current.state.review.status).toBe('ready');
  });
  act(() => {
    flow.result.current.approve();
  });
  await waitFor(() => {
    expect(flow.result.current.state.apply.status).toBe('failed');
  });
  return { port, flow };
}

describe('resuming a lost transfer in place', () => {
  it('resumes once, shows it is on its way, and cancels it when the user leaves', async () => {
    const { port, flow } = await lostTransfer();
    const resume = vi.spyOn(port, 'resume').mockReturnValue(new Promise(() => undefined));
    const cancel = vi.spyOn(port, 'cancel');
    const session = flow.result.current.state.session;

    act(() => {
      flow.result.current.retry();
    });
    act(() => {
      flow.result.current.retry();
    });
    expect(resume).toHaveBeenCalledTimes(1);
    const lost = { ...emptySession('s'), link: 'lost' as const };
    expect(receiveProgressModel(flow.result.current.state, lost, null, 'ana')).toMatchObject({
      resuming: true,
      resumeError: null,
    });

    act(() => {
      flow.result.current.abandon();
    });
    expect(cancel).toHaveBeenCalledWith(session);
  });

  it('says why a resume failed on the progress screen', async () => {
    const { port, flow } = await lostTransfer();
    vi.spyOn(port, 'resume').mockRejectedValue(LOST);
    act(() => {
      flow.result.current.retry();
    });
    const lost = { ...emptySession('s'), link: 'lost' as const };
    await waitFor(() => {
      expect(receiveProgressModel(flow.result.current.state, lost, null, 'ana')).toMatchObject({
        status: 'transfer',
        resuming: false,
        resumeError: LOST,
      });
    });
  });
});

describe('Accept all', () => {
  it('accept_all_keeps_a_left_pick_left_in_the_answers', async () => {
    const port = createFakePort({ scenario: 'picks', stepMs: 0 });
    const flow = renderHook(() => useReceiveFlow(port, {}, DEFAULTS));
    act(() => {
      flow.result.current.start('lan');
      flow.result.current.setCode('7-acid-lemon');
    });
    act(() => {
      flow.result.current.open();
    });
    await waitFor(() => {
      expect(flow.result.current.state.review.status).toBe('ready');
    });
    act(() => {
      flow.result.current.answer('raw:home/notes.txt', 'leave');
    });
    await waitFor(() => {
      expect(flow.result.current.state.review.status).toBe('ready');
    });
    act(() => {
      flow.result.current.acceptSafePicks();
    });
    expect(flow.result.current.state.answers).toMatchObject({
      'raw:home/notes.txt': 'leave',
      'raw:home/projects/site': 'bring',
    });
  });
});
