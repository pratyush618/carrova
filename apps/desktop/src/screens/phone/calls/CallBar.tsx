import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { CallWindowView } from '../../../bindings/CallWindowView';
import { Button, Icon } from '../../../ui';
import './calls.css';

export interface CallBarIntents {
  readonly onAnswer: () => void;
  readonly onDecline: () => void;
  readonly onMute: (on: boolean) => void;
  readonly onEnd: () => void;
}

/**
 * Home's bar while a phone's call rings or runs, or just after the phone
 * left mid-call ("<Phone> left"). Nothing otherwise.
 */
export function CallBar({
  view,
  who,
  phone,
  left,
  on,
}: {
  readonly view: CallWindowView | null;
  readonly who: string | null;
  readonly phone: string;
  /** The phone left mid-call (shown while no call runs). */
  readonly left: boolean;
  readonly on: CallBarIntents;
}) {
  const { t } = useTranslation();
  // Only `text` is announced: a running timer outside it doesn't re-read the bar every second.
  const bar = (text: string, acts: ReactNode = null, timer: string | null = null) => (
    <div className="s-callbar">
      <Icon name="call" />
      <span className="s-callbar__text">
        <span role="status">{text}</span>
        {timer !== null && ` · ${timer}`}
      </span>
      {acts}
    </div>
  );
  switch (view?.kind) {
    case 'ringing':
      return bar(
        t('calls.bar.ringing', { who: who ?? view.caller.number }),
        <>
          <Button size="sm" kind="danger" onClick={on.onDecline}>
            {t('calls.window.decline')}
          </Button>
          <Button size="sm" kind="primary" onClick={on.onAnswer}>
            {t('calls.window.answer')}
          </Button>
        </>,
      );
    case 'active': {
      const where =
        view.audio === 'laptop' ? t('calls.bar.here') : t('calls.bar.onPhone', { phone });
      // "Calling…" until the phone reports a timer: it doesn't tick, so it is announced.
      const dialling = view.timer === null ? [t('calls.window.dialling')] : [];
      return bar(
        [who ?? t('calls.bar.active'), ...dialling, where].join(' · '),
        <>
          <Button
            size="sm"
            icon={view.muted ? 'micOff' : 'mic'}
            aria-pressed={view.muted}
            onClick={() => {
              on.onMute(!view.muted);
            }}
          >
            {t(view.muted ? 'calls.window.unmute' : 'calls.window.mute')}
          </Button>
          <Button size="sm" kind="danger" onClick={on.onEnd}>
            {t('calls.window.end')}
          </Button>
        </>,
        view.timer,
      );
    }
    default:
      return left ? bar(t('calls.bar.left', { phone })) : null;
  }
}
