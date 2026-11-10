import type { SessionProgress } from '../state/session';
import type { Controllers } from '../controllers/Controllers';
import type { FlowPosition } from '../nav/flows';
import type { View } from '../nav/navMachine';
import { receiveStage } from '../../flows/receive/receiveFlow';
import { sendStep } from '../../flows/send/sendProgressViews';

/**
 * The step a flow shows comes from its controller (one source of truth);
 * navigation only records which flow is open.
 */
export function livePosition(
  at: FlowPosition,
  c: Controllers,
  sending: SessionProgress,
): FlowPosition {
  switch (at.flow) {
    case 'send':
      return { flow: 'send', step: sendStep(c.send.state, sending) };
    case 'save':
      return { flow: 'save', step: c.save.state.step };
    case 'receive': {
      const stage = receiveStage(c.receive.state);
      return { flow: 'receive', step: stage === 'entry' ? 'code' : stage };
    }
    case 'open': {
      const stage = receiveStage(c.receive.state);
      return { flow: 'open', step: stage === 'entry' ? 'file' : stage };
    }
    case 'undo':
      return { flow: 'undo', step: c.undo.rollback.status === 'idle' ? 'confirm' : 'result' };
  }
}

export function liveView(view: View, c: Controllers, sending: SessionProgress): View {
  return view.kind === 'flow' ? { kind: 'flow', at: livePosition(view.at, c, sending) } : view;
}

/** Stable key per screen: focus moves and the window resets when it changes. */
export const viewKey = (view: View): string =>
  view.kind === 'flow'
    ? `${view.at.flow}/${view.at.step}`
    : view.kind === 'hub'
      ? view.page
      : view.kind;
