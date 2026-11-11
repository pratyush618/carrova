import type {
  BridgeError,
  PickOutcome,
  ScanResult,
  Selection,
  SelectionView,
  SendReport,
  SessionId,
} from '../../engine/types';
import { idle, remoteReducer, type Remote, type RemoteAction } from '../remote';

/** Auto: the apps the scan found. Manual: only the files and folders the user picks. */
export type SendMode = 'auto' | 'manual';

/** Each mode's latest engine reply; `null` until that mode was first shown. */
export type ByMode = Readonly<Record<SendMode, ScanResult | null>>;

/**
 * The sending side: scan → the user's selection → pairing and sending → done.
 * Saving to a file shares the scan and selection part. Auto and Manual keep
 * a selection each; `scan`, `selection` and `view` are the visible mode's,
 * and only that one moves.
 */
export interface SendFlowState {
  readonly mode: SendMode;
  readonly byMode: ByMode;
  /** Each mode's failed first reply, kept so showing that mode again shows the failure. */
  readonly failures: Readonly<Record<SendMode, BridgeError | null>>;
  /** Picked paths the engine is still measuring. */
  readonly pending: readonly string[];
  /** What the latest pick did with each path (added, absorbed, refused). */
  readonly outcomes: readonly PickOutcome[];
  /** The export session (both modes): opened by the scan, or an interrupted one being resumed. */
  readonly session: SessionId | null;
  /** The visible mode's latest reply: the scan, the session's selection and its view. */
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

/** `mode` (absent: the visible one) is the mode a reply belongs to. */
export type SendFlowAction =
  | {
      readonly type: 'scan';
      readonly mode?: SendMode;
      readonly action: RemoteAction<ScanResult>;
    }
  | { readonly type: 'selected'; readonly mode?: SendMode; readonly result: ScanResult }
  | { readonly type: 'mode'; readonly mode: SendMode }
  | { readonly type: 'measuring'; readonly paths: readonly string[] }
  | {
      readonly type: 'picked';
      readonly paths: readonly string[];
      /** The Manual selection after the pick; `null` when the pick failed. */
      readonly result: ScanResult | null;
      readonly outcomes: readonly PickOutcome[];
    }
  | { readonly type: 'selectFailed'; readonly error: BridgeError }
  | { readonly type: 'resumed'; readonly session: SessionId }
  | { readonly type: 'run'; readonly action: RemoteAction<SendReport> }
  | { readonly type: 'back' }
  | { readonly type: 'reset'; readonly mode?: SendMode };

export const initialSendFlow: SendFlowState = {
  mode: 'auto',
  byMode: { auto: null, manual: null },
  failures: { auto: null, manual: null },
  pending: [],
  outcomes: [],
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

/** `state` with `result` as `mode`'s reply (shown when `mode` is). */
function store(state: SendFlowState, mode: SendMode, result: ScanResult): SendFlowState {
  const next = { ...state, byMode: { ...state.byMode, [mode]: result } };
  return mode === state.mode
    ? withResult({ ...next, scan: { status: 'ready', value: result } }, result)
    : next;
}

/** `state` showing `mode`: its reply if it has one, else nothing yet. */
function show(state: SendFlowState, mode: SendMode): SendFlowState {
  const result = state.byMode[mode];
  const error = state.failures[mode];
  const next = { ...state, mode, selectError: null };
  if (result !== null)
    return withResult({ ...next, scan: { status: 'ready', value: result } }, result);
  const scan: Remote<ScanResult> = error === null ? idle : { status: 'failed', error, last: null };
  return { ...next, scan, selection: null, view: null };
}

export function sendFlowReducer(state: SendFlowState, action: SendFlowAction): SendFlowState {
  switch (action.type) {
    case 'scan': {
      const mode = action.mode ?? state.mode;
      const { action: step } = action;
      if (step.type === 'ok') return store(state, mode, step.value);
      const failure = step.type === 'fail' ? step.error : null;
      const next = { ...state, failures: { ...state.failures, [mode]: failure } };
      return mode === state.mode ? { ...next, scan: remoteReducer(state.scan, step) } : next;
    }
    case 'selected':
      return store(state, action.mode ?? state.mode, action.result);
    case 'mode':
      return show(state, action.mode);
    case 'measuring':
      return { ...state, pending: [...state.pending, ...action.paths] };
    case 'picked': {
      const next = {
        ...state,
        pending: state.pending.filter((p) => !action.paths.includes(p)),
        outcomes: action.outcomes,
      };
      return action.result === null ? next : store(next, 'manual', action.result);
    }
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
      return { ...initialSendFlow, mode: action.mode ?? 'auto' };
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

/** Whether a send is running (pairing or moving): forwarded picks must wait. */
export const sendBusy = (state: SendFlowState): boolean => state.run.status === 'loading';
