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
  const bar = (text: string, acts: ReactNode = null) => (
    <div className="s-callbar" role="status">
      <Icon name="call" />
      <span className="s-callbar__text">{text}</span>
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
      return bar(
        [who ?? t('calls.bar.active'), view.timer ?? t('calls.window.dialling'), where].join(' · '),
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
      );
    }
    default:
      return left ? bar(t('calls.bar.left', { phone })) : null;
  }
}
