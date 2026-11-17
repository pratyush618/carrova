import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { AudioSide } from '../../../bindings/AudioSide';
import { Button } from '../../../ui';
import { initials } from '../messages/ThreadList';
import { Keypad } from './Keypad';
import './calls.css';

/** The live call the window shows (`CallWindowView` `active`). */
export interface InCallView {
  readonly timer: string;
  readonly muted: boolean;
  readonly audio: AudioSide;
  /** Offer the keypad (tones go through this computer's audio, so only while it is here). */
  readonly keypad: boolean;
}

/**
 * A call in progress: who, how long, where the sound is, and mute,
 * keypad, moving the sound and ending it. Closing the main window never
 * ends it.
 */
export function InCall({
  who,
  name,
  phone,
  view,
  on,
}: {
  /** The caller's name or number; `null` for a call made on the phone. */
  readonly who: string | null;
  /** The caller's contact name (`null`: unknown), for the initials. */
  readonly name: string | null;
  readonly phone: string;
  readonly view: InCallView;
  readonly on: {
    readonly onMute: (on: boolean) => void;
    readonly onTone: (digit: string) => void;
    readonly onAudioTo: (side: AudioSide) => void;
    readonly onEnd: () => void;
  };
}) {
  const { t } = useTranslation();
  const [keys, setKeys] = useState(false);
  const here = view.audio === 'laptop';
  return (
    <main className="s-call">
      <span className="s-call__av" aria-hidden="true">
        {initials(name)}
      </span>
      <h1 className="s-call__who">{who ?? t('calls.window.onCall')}</h1>
      <p className="s-call__timer" aria-live="off">
        {view.muted ? `${t('calls.window.muted')} · ${view.timer}` : view.timer}
      </p>
      <p className="s-call__via" aria-live="polite">
        {here ? t('calls.window.viaLaptop', { phone }) : t('calls.window.viaPhone', { phone })}
      </p>
      {view.keypad && keys && <Keypad onTone={on.onTone} />}
      <div className="s-call__acts">
        <Button
          icon={view.muted ? 'micOff' : 'mic'}
          aria-pressed={view.muted}
          onClick={() => {
            on.onMute(!view.muted);
          }}
        >
          {t(view.muted ? 'calls.window.unmute' : 'calls.window.mute')}
        </Button>
        {view.keypad && (
          <Button
            icon="keypad"
            aria-pressed={keys}
            onClick={() => {
              setKeys(!keys);
            }}
          >
            {t('calls.window.keypad')}
          </Button>
        )}
        <Button
          icon={here ? 'phone' : 'laptop'}
          onClick={() => {
            on.onAudioTo(here ? 'phone' : 'laptop');
          }}
        >
          {t(here ? 'calls.window.toPhone' : 'calls.window.toLaptop')}
        </Button>
        <Button kind="danger" icon="callEnd" onClick={on.onEnd}>
          {t('calls.window.end')}
        </Button>
      </div>
    </main>
  );
}
