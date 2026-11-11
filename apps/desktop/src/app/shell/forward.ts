import { useEffect, useRef, useState } from 'react';
import { usePort } from '../../engine/port';
import type { SessionId } from '../../engine/types';
import type { Remote } from '../../flows/remote';
import type { SaveFlowState } from '../../flows/archive/saveFlow';
import { receiveStage, type ReceiveFlowState } from '../../flows/receive/receiveFlow';
import { sendBusy, type SendFlowState } from '../../flows/send/sendFlow';
import type { SendController } from '../../flows/send/useSendFlow';
import { useControllers } from '../controllers/Controllers';
import type { NavEvent, View } from '../nav/navMachine';
import { useSendRequests } from '../state/EngineState';

/** The flows a forwarded send must not disturb. */
export interface MoveStates {
  readonly send: SendFlowState;
  readonly save: SaveFlowState;
  readonly receive: ReceiveFlowState;
}

/**
 * What paths sent from the file manager do now: refused while a move runs
 * (sending, saving, or receiving from pairing to the last step), queued for
 * the Manual send still opening, added to the selection open on Send, else
 * a fresh Manual send.
 */
export function forwardOf(
  { send, save, receive }: MoveStates,
  view: View,
): 'refuse' | 'queue' | 'add' | 'start' {
  const stage = receiveStage(receive);
  const receiving =
    receive.code.status === 'connecting' ||
    receive.file.status === 'opening' ||
    stage === 'review' ||
    stage === 'progress';
  if (sendBusy(send) || save.save.status === 'loading' || receiving) return 'refuse';
  if (send.mode === 'manual' && send.session === null && send.scan.status === 'loading') {
    return 'queue';
  }
  const onSend = view.kind === 'flow' && view.at.flow === 'send';
  return onSend && send.session !== null && send.step === 'found' ? 'add' : 'start';
}

/** What a forwarded request is decided against, read when its paths arrive. */
export interface ForwardTarget {
  readonly states: MoveStates;
  readonly view: View;
  readonly send: Pick<SendController, 'start' | 'addPaths'>;
  readonly go: (event: NavEvent) => void;
}

/**
 * Hands requests from the file manager over one at a time: each is decided
 * ({@link forwardOf}) and dispatched before the next is taken. Paths that
 * arrive while a Manual send opens wait for it (`settle`), even before the
 * app has re-rendered the send it just started.
 */
export function forwarder(
  take: () => Promise<string[]>,
  now: () => ForwardTarget,
  refused: (shown: boolean) => void,
) {
  let drains: Promise<void> = Promise.resolve();
  /** Paths for the Manual send that is opening; `null` while none is. */
  let waiting: string[] | null = null;
  const handle = (paths: readonly string[]) => {
    if (paths.length === 0) return;
    const { states, view, send, go } = now();
    const step = waiting === null ? forwardOf(states, view) : 'queue';
    refused(step === 'refuse');
    if (step === 'queue') waiting = [...(waiting ?? []), ...paths];
    if (step === 'add') send.addPaths(paths);
    if (step === 'start') {
      waiting = [];
      send.start('manual', paths);
      go({ type: 'startFlow', flow: 'send' });
    }
  };
  return {
    /** Take the next request; a failed take has nothing to open (the next request asks again). */
    drain: (): Promise<void> => {
      drains = drains.then(take).then(handle, () => undefined);
      return drains;
    },
    /** The send's session and first reply now: hand the waiting paths to it once it opened. */
    settle: (session: SessionId | null, scan: Remote<unknown>['status']) => {
      if (waiting === null) return;
      if (session !== null) {
        const paths = waiting;
        waiting = null;
        if (paths.length > 0) now().send.addPaths(paths);
      } else if (scan !== 'loading') {
        // Failed to open, or closed while opening: the next request starts afresh.
        waiting = null;
      }
    },
  };
}

/**
 * Takes paths sent from the file manager (at startup, then on each
 * `send_requested`) and opens them in a Manual send, or refuses them while a
 * move runs. `refused`: show "Finish or cancel the current move first".
 */
export function useForwardedSends(go: (event: NavEvent) => void, view: View) {
  const port = usePort();
  const requests = useSendRequests();
  const { send, save, receive } = useControllers();
  const [refused, setRefused] = useState(false);
  const latest = useRef({ port, send, save, receive, view, go });
  useEffect(() => {
    latest.current = { port, send, save, receive, view, go };
  });
  const forwarding = useRef<ReturnType<typeof forwarder> | null>(null);
  useEffect(() => {
    forwarding.current ??= forwarder(
      () => latest.current.port.takeSendRequest(),
      () => {
        const now = latest.current;
        const states = { send: now.send.state, save: now.save.state, receive: now.receive.state };
        return { states, view: now.view, send: now.send, go: now.go };
      },
      setRefused,
    );
    // Never cancelled: the bridge hands each path over once, so a late answer still counts.
    void forwarding.current.drain();
  }, [port, requests]);
  const { session: sending, scan } = send.state;
  useEffect(() => {
    forwarding.current?.settle(sending, scan.status);
  }, [sending, scan.status]);
  return {
    refused,
    dismiss: () => {
      setRefused(false);
    },
  };
}
