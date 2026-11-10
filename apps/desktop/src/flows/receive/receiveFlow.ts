import { ENGINE_DEFAULTS } from '../../bindings/constants';
import type {
  BridgeError,
  Checklist,
  MoveDefaults,
  PlanOptions,
  PlanReview,
  SessionId,
} from '../../engine/types';
import type { DecisionAnswer } from '../../screens/receive/model';
import { idle, remoteReducer, type Remote, type RemoteAction } from '../remote';
import {
  codeFailure,
  fileFailure,
  initialCode,
  initialFile,
  type CodeEntry,
  type FileEntry,
} from './entry';
import { suggestedPlanOptions } from './planOptions';
import { nothingAsked, rememberAsked, type Answers, type Asked } from './reviewViews';

/** Receiving over the LAN or from a file: entry → review → apply → arrived. */
export interface ReceiveFlowState {
  readonly origin: 'lan' | 'file';
  readonly code: CodeEntry;
  readonly file: FileEntry;
  readonly session: SessionId | null;
  /**
   * Options of the session's first plan (the suggestions, or a resumed session's
   * own). Later changes go to the engine as edits; the plan carries the result.
   */
  readonly options: PlanOptions;
  readonly answers: Answers;
  /** Questions the engine raised during this review (rows stay after they're answered). */
  readonly asked: Asked;
  readonly review: Remote<PlanReview>;
  /** Approve + apply (transfer, stage, put in place). */
  readonly apply: Remote<Checklist>;
}

export type ReceiveFlowAction =
  | {
      readonly type: 'start';
      readonly origin: 'lan' | 'file';
      /** The user's suggested defaults from Settings. */
      readonly defaults: MoveDefaults;
    }
  | { readonly type: 'code'; readonly value: string }
  | { readonly type: 'address'; readonly value: string }
  | { readonly type: 'path'; readonly value: string }
  | { readonly type: 'passphrase'; readonly value: string }
  | { readonly type: 'opening' }
  | { readonly type: 'openFailed'; readonly error: BridgeError }
  | { readonly type: 'opened'; readonly session: SessionId }
  /** A session picked up again, with the options it last planned with (if any). */
  | { readonly type: 'resumed'; readonly session: SessionId; readonly options: PlanOptions | null }
  | { readonly type: 'plan'; readonly action: RemoteAction<PlanReview> }
  | { readonly type: 'answer'; readonly key: string; readonly answer: DecisionAnswer }
  | { readonly type: 'apply'; readonly action: RemoteAction<Checklist> };

export const initialReceiveFlow: ReceiveFlowState = {
  origin: 'lan',
  code: initialCode,
  file: initialFile,
  session: null,
  options: ENGINE_DEFAULTS.plan_options,
  answers: {},
  asked: nothingAsked,
  review: idle,
  apply: idle,
};

export function receiveFlowReducer(
  state: ReceiveFlowState,
  action: ReceiveFlowAction,
): ReceiveFlowState {
  switch (action.type) {
    case 'start':
      return {
        ...initialReceiveFlow,
        origin: action.origin,
        options: suggestedPlanOptions(action.defaults),
      };
    case 'code':
      return { ...state, code: { ...state.code, value: action.value } };
    case 'address':
      return { ...state, code: { ...state.code, address: action.value } };
    case 'path':
      return { ...state, file: { ...state.file, path: action.value } };
    case 'passphrase':
      return { ...state, file: { ...state.file, passphrase: action.value } };
    case 'opening':
      return state.origin === 'lan'
        ? { ...state, code: { ...state.code, status: 'connecting', error: null } }
        : { ...state, file: { ...state.file, status: 'opening', error: null } };
    case 'openFailed':
      return state.origin === 'lan'
        ? { ...state, code: codeFailure(state.code, action.error) }
        : { ...state, file: fileFailure(state.file, action.error) };
    case 'opened':
      return { ...state, session: action.session, asked: nothingAsked, review: idle, apply: idle };
    case 'resumed':
      // The session's own options, not the suggestions: its choices and approval carry over.
      return {
        ...state,
        session: action.session,
        options: action.options ?? state.options,
        asked: nothingAsked,
        review: idle,
        apply: idle,
      };
    case 'plan':
      return {
        ...state,
        review: remoteReducer(state.review, action.action),
        asked:
          action.action.type === 'ok'
            ? rememberAsked(state.asked, action.action.value)
            : state.asked,
      };
    case 'answer':
      return { ...state, answers: { ...state.answers, [action.key]: action.answer } };
    case 'apply':
      return { ...state, apply: remoteReducer(state.apply, action.action) };
  }
}

/** The step a receive or open flow shows. Entry is `code` (LAN) or `file`. */
export function receiveStage(state: ReceiveFlowState): 'entry' | 'review' | 'progress' | 'done' {
  if (state.session === null) return 'entry';
  switch (state.apply.status) {
    case 'idle':
      return 'review';
    case 'loading':
    case 'failed':
      return 'progress';
    case 'ready':
      return 'done';
  }
}
