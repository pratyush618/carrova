import { useCallback, useMemo, useReducer } from 'react';
import { toBridgeError } from '../../engine/errors';
import type { DesktopHost } from '../../engine/host';
import type { KdfStrength, CarrovaPort } from '../../engine/types';
import type { SendController } from '../send/useSendFlow';
import { initialSaveFlow, saveFlowReducer, type SaveFlowState } from './saveFlow';

/** Suggested file name; the user can type or pick any other. */
export const DEFAULT_ARCHIVE_NAME = 'carrova-setup.carrova';

export interface SaveController {
  readonly state: SaveFlowState;
  readonly start: () => void;
  /** Save the selection already made for sending (no new scan). */
  readonly startFromSelection: () => void;
  readonly toLock: () => void;
  readonly back: () => void;
  readonly setPassphrase: (value: string) => void;
  readonly setConfirm: (value: string) => void;
  readonly setPath: (value: string) => void;
  readonly setStrength: (value: KdfStrength) => void;
  /** Present only when the host has a native "save as" dialog. */
  readonly choosePath: (() => void) | null;
  readonly save: () => void;
}

/** The effect layer of saving; "what goes in" is the send controller's selection. */
export function useSaveFlow(
  port: CarrovaPort,
  send: SendController,
  host: DesktopHost,
): SaveController {
  const [state, dispatch] = useReducer(saveFlowReducer, initialSaveFlow(DEFAULT_ARCHIVE_NAME));
  const { start: scan, confirmSelection } = send;
  const session = send.state.session;

  const start = useCallback(() => {
    dispatch({ type: 'reset', path: DEFAULT_ARCHIVE_NAME });
    scan();
  }, [scan]);

  const toLock = useCallback(() => {
    void confirmSelection().then((ok) => {
      if (ok) dispatch({ type: 'step', step: 'lock' });
    });
  }, [confirmSelection]);

  const { pickSavePath } = host;
  const choosePath = useMemo(
    () =>
      pickSavePath === undefined
        ? null
        : () => {
            void pickSavePath(DEFAULT_ARCHIVE_NAME).then((path) => {
              if (path !== null) dispatch({ type: 'path', value: path });
            });
          },
    [pickSavePath],
  );

  const save = useCallback(() => {
    if (session === null) return;
    dispatch({ type: 'save', action: { type: 'start' } });
    port.exportArchive(session, state.path, state.passphrase, state.strength).then(
      (value) => {
        dispatch({ type: 'save', action: { type: 'ok', value } });
      },
      (reason: unknown) => {
        dispatch({ type: 'save', action: { type: 'fail', error: toBridgeError(reason) } });
      },
    );
  }, [port, session, state.path, state.passphrase, state.strength]);

  return useMemo(
    () => ({
      state,
      start,
      startFromSelection: () => {
        dispatch({ type: 'reset', path: DEFAULT_ARCHIVE_NAME });
      },
      toLock,
      back: () => {
        dispatch({ type: 'step', step: 'what' });
      },
      setPassphrase: (value: string) => {
        dispatch({ type: 'passphrase', value });
        void port.passphraseStrength(value).then((strength) => {
          dispatch({ type: 'rated', passphrase: value, strength });
        });
      },
      setConfirm: (value: string) => {
        dispatch({ type: 'confirm', value });
      },
      setPath: (value: string) => {
        dispatch({ type: 'path', value });
      },
      setStrength: (value: KdfStrength) => {
        dispatch({ type: 'strength', value });
      },
      choosePath,
      save,
    }),
    [state, start, toLock, choosePath, save, port],
  );
}
