import { useEffect, useRef, useState } from 'react';
import { usePort } from '../../engine/port';
import type { SaveFlowState } from '../../flows/archive/saveFlow';
import { receiveStage, type ReceiveFlowState } from '../../flows/receive/receiveFlow';
import { sendBusy, type SendFlowState } from '../../flows/send/sendFlow';
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
 * (sending, saving, or receiving from pairing to the last step), added to
 * the selection open on Send, else a fresh Manual send.
 */
export function forwardOf(
  { send, save, receive }: MoveStates,
  view: View,
): 'refuse' | 'add' | 'start' {
  const stage = receiveStage(receive);
  const receiving =
    receive.code.status === 'connecting' ||
    receive.file.status === 'opening' ||
    stage === 'review' ||
    stage === 'progress';
  if (sendBusy(send) || save.save.status === 'loading' || receiving) return 'refuse';
  const onSend = view.kind === 'flow' && view.at.flow === 'send';
  return onSend && send.session !== null && send.step === 'found' ? 'add' : 'start';
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
  const latest = useRef({ send, save, receive, view, go });
  useEffect(() => {
    latest.current = { send, save, receive, view, go };
  });
  useEffect(() => {
    // Never cancelled: the bridge hands each path over once, so a late answer still counts.
    port.takeSendRequest().then(
      (paths) => {
        if (paths.length === 0) return;
        const now = latest.current;
        const states = { send: now.send.state, save: now.save.state, receive: now.receive.state };
        const step = forwardOf(states, now.view);
        setRefused(step === 'refuse');
        if (step === 'add') now.send.addPaths(paths);
        if (step === 'start') {
          now.send.start('manual', paths);
          now.go({ type: 'startFlow', flow: 'send' });
        }
      },
      // A failed drain has nothing to open; the next request asks again.
      () => undefined,
    );
  }, [port, requests]);
  return {
    refused,
    dismiss: () => {
      setRefused(false);
    },
  };
}
