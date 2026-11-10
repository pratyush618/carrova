import type { FlowStep } from '../../app/nav/flows';
import type { PassphraseStrength } from '../../bindings/PassphraseStrength';
import type { ArchiveSummary, KdfStrength } from '../../engine/types';
import type { SavedModel, SaveLockModel } from '../../screens/file/model';
import { idle, remoteReducer, type Remote, type RemoteAction } from '../remote';
import { strongEnough } from './passphrase';

/** Saving to an encrypted file, after the shared "what goes in" step. */
export interface SaveFlowState {
  readonly step: FlowStep<'save'>;
  readonly passphrase: string;
  /** The engine's rating of the passphrase typed so far (`null` until it answers). */
  readonly rated: { readonly passphrase: string; readonly strength: PassphraseStrength } | null;
  readonly confirm: string;
  readonly path: string;
  /** How hard the passphrase is to guess (the engine's levels; `standard` suggested). */
  readonly strength: KdfStrength;
  readonly save: Remote<ArchiveSummary>;
}

export type SaveFlowAction =
  | { readonly type: 'step'; readonly step: FlowStep<'save'> }
  | { readonly type: 'passphrase'; readonly value: string }
  | { readonly type: 'rated'; readonly passphrase: string; readonly strength: PassphraseStrength }
  | { readonly type: 'confirm'; readonly value: string }
  | { readonly type: 'path'; readonly value: string }
  | { readonly type: 'strength'; readonly value: KdfStrength }
  | { readonly type: 'save'; readonly action: RemoteAction<ArchiveSummary> }
  | { readonly type: 'reset'; readonly path: string };

export const initialSaveFlow = (path: string): SaveFlowState => ({
  step: 'what',
  passphrase: '',
  rated: null,
  confirm: '',
  path,
  strength: 'standard',
  save: idle,
});

export function saveFlowReducer(state: SaveFlowState, action: SaveFlowAction): SaveFlowState {
  switch (action.type) {
    case 'step':
      return { ...state, step: action.step };
    case 'passphrase':
      return { ...state, passphrase: action.value };
    case 'rated':
      // Only the rating of what is typed now counts; late answers are dropped.
      return action.passphrase === state.passphrase
        ? { ...state, rated: { passphrase: action.passphrase, strength: action.strength } }
        : state;
    case 'confirm':
      return { ...state, confirm: action.value };
    case 'path':
      return { ...state, path: action.value };
    case 'strength':
      return { ...state, strength: action.value };
    case 'save': {
      const save = remoteReducer(state.save, action.action);
      return { ...state, save, step: action.action.type === 'ok' ? 'done' : state.step };
    }
    case 'reset':
      return initialSaveFlow(action.path);
  }
}

/** The engine's rating of the current passphrase, once it has answered for it. */
function strengthNow(state: SaveFlowState): PassphraseStrength | null {
  return state.rated?.passphrase === state.passphrase ? state.rated.strength : null;
}

/** Saving is allowed with a strong-enough, confirmed passphrase and a destination. */
export function canSave(state: SaveFlowState): boolean {
  const strength = strengthNow(state);
  return (
    strength !== null &&
    strongEnough(strength) &&
    state.confirm === state.passphrase &&
    state.path.trim() !== '' &&
    state.save.status !== 'loading'
  );
}

export function saveLockModel(state: SaveFlowState, canChoose: boolean): SaveLockModel {
  return {
    passphrase: state.passphrase,
    confirm: state.confirm,
    // The previous rating stays on the meter until the engine rates the new text.
    strength: strengthNow(state) ?? state.rated?.strength ?? 'weak',
    path: state.path,
    kdf: state.strength,
    canChoose,
    saving: state.save.status === 'loading',
    canSave: canSave(state),
    error: state.save.status === 'failed' ? state.save.error : null,
  };
}

export function savedModel(state: SaveFlowState): SavedModel | null {
  return state.save.status === 'ready' ? { path: state.path, bytes: state.save.value.bytes } : null;
}
