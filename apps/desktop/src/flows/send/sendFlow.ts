import type {
  BridgeError,
  ScanResult,
  Selection,
  SelectionView,
  SendReport,
  SessionId,
} from '../../engine/types';
import { idle, remoteReducer, type Remote, type RemoteAction } from '../remote';

/**
 * The sending side: scan → the user's selection → pairing and sending → done.
 * Saving to a file shares the scan and selection part.
 */
export interface SendFlowState {
  /** The export session: opened by the scan, or an interrupted one being resumed. */
  readonly session: SessionId | null;
  /** The engine's latest reply: the scan, the session's selection and its view. */
  readonly scan: Remote<ScanResult>;
  /** The session's selection: the engine's suggestion, then each edit's result. */
  readonly selection: Selection | null;
  /** The engine's view of that selection (issues, totals). */
  readonly view: SelectionView | null;
  /** The engine refused the latest edit (the selection is unchanged). */
  readonly selectError: BridgeError | null;
  readonly step: 'found' | 'pairing' | 'done';
  readonly run: Remote<SendReport>;
}

export type SendFlowAction =
  | { readonly type: 'scan'; readonly action: RemoteAction<ScanResult> }
  | { readonly type: 'selected'; readonly result: ScanResult }
  | { readonly type: 'selectFailed'; readonly error: BridgeError }
  | { readonly type: 'resumed'; readonly session: SessionId }
  | { readonly type: 'run'; readonly action: RemoteAction<SendReport> }
  | { readonly type: 'back' }
  | { readonly type: 'reset' };

export const initialSendFlow: SendFlowState = {
  session: null,
  scan: idle,
  selection: null,
  view: null,
  selectError: null,
  step: 'found',
  run: idle,
};

/** `state` holding the engine's reply `result`. */
function withResult(state: SendFlowState, result: ScanResult): SendFlowState {
  const { session, selection, view } = result;
  return { ...state, session, selection, view, selectError: null };
}

export function sendFlowReducer(state: SendFlowState, action: SendFlowAction): SendFlowState {
  switch (action.type) {
    case 'scan': {
      const scan = remoteReducer(state.scan, action.action);
      return action.action.type === 'ok'
        ? withResult({ ...state, scan }, action.action.value)
        : { ...state, scan };
    }
    case 'selected':
      return withResult(
        { ...state, scan: { status: 'ready', value: action.result } },
        action.result,
      );
    case 'selectFailed':
      return { ...state, selectError: action.error };
    case 'resumed':
      return { ...initialSendFlow, session: action.session };
    case 'run': {
      const run = remoteReducer(state.run, action.action);
      const { type } = action.action;
      const step = type === 'start' ? 'pairing' : type === 'ok' ? 'done' : state.step;
      return { ...state, run, step };
    }
    case 'back':
      return { ...state, step: 'found', run: idle };
    case 'reset':
      return initialSendFlow;
  }
}

/** Whether the selection may go ahead: scanned, sent to the engine, and without issues. */
export function selectionReady(state: SendFlowState): boolean {
  return (
    state.selection !== null &&
    state.view !== null &&
    state.view.issues.length === 0 &&
    state.selectError === null
  );
}
