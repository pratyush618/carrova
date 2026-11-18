import { useCallback, useEffect, useRef, useState } from 'react';
import type { CallSetupStep } from '../../bindings/CallSetupStep';
import { toBridgeError } from '../../engine/errors';
import type { CarrovaPort } from '../../engine/types';

/** Why the last step didn't go through: it failed, or the user dismissed the password prompt. */
export type SetupNote = 'failed' | 'cancelled';

/** The screen this computer's call-audio state calls for (ready: check the sound first). */
export const screenFor = (status: CallSetupStep): CallSetupStep =>
  status === 'done' ? 'test' : status;

/** The Calls setup wizard: the user's own steps, and the ones the computer's state decides. */
export interface CallsSetup {
  readonly step: CallSetupStep;
  /** A check, install or audio change runs. */
  readonly busy: boolean;
  /** The package was installed in this wizard. */
  readonly installed: boolean;
  /** The user agreed to the audio-settings change. */
  readonly agreed: boolean;
  readonly note: SetupNote | null;
  /** One of the user's own steps (intro, phone, test, done). */
  readonly go: (step: CallSetupStep) => void;
  /** Ask the computer where setup stands, and show that. */
  readonly check: () => void;
  readonly install: () => void;
  readonly agree: (agreed: boolean) => void;
  /** Make the audio change, only once agreed; then check again. */
  readonly configure: () => void;
}

export function useCallsSetup(port: CarrovaPort): CallsSetup {
  const [step, setStep] = useState<CallSetupStep>('intro');
  const [busy, setBusy] = useState(false);
  const [installed, setInstalled] = useState(false);
  const [agreed, setAgreed] = useState(false);
  const [note, setNote] = useState<SetupNote | null>(null);
  const live = useRef(true);
  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
    };
  }, []);
  const run = useCallback(<T>(work: () => Promise<T>, then: (value: T) => void) => {
    setBusy(true);
    setNote(null);
    work().then(
      (value) => {
        if (!live.current) return;
        setBusy(false);
        then(value);
      },
      (reason: unknown) => {
        if (!live.current) return;
        setBusy(false);
        setNote(toBridgeError(reason).code === 'cancelled' ? 'cancelled' : 'failed');
      },
    );
  }, []);
  const check = useCallback(() => {
    run(
      () => port.callAudioStep(),
      (status) => {
        setStep(screenFor(status));
      },
    );
  }, [port, run]);
  return {
    step,
    busy,
    installed,
    agreed,
    note,
    go: (next) => {
      setNote(null);
      setStep(next);
    },
    check,
    install: () => {
      run(
        () => port.installCallAudio(),
        () => {
          setInstalled(true);
        },
      );
    },
    agree: setAgreed,
    configure: () => {
      if (agreed) run(() => port.setCallAudioConfig(true), check);
    },
  };
}
