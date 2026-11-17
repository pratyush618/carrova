import type { AudioSide } from '../../../bindings/AudioSide';
import type { CallWindowView } from '../../../bindings/CallWindowView';
import { IncomingCall } from './IncomingCall';
import { InCall } from './InCall';
import { MissedCall } from './MissedCall';

export interface CallWindowIntents {
  readonly onAnswer: () => void;
  readonly onDecline: () => void;
  readonly onMute: (on: boolean) => void;
  readonly onTone: (digit: string) => void;
  readonly onAudioTo: (side: AudioSide) => void;
  readonly onEnd: () => void;
  readonly onCallBack: () => void;
  /** Close the window (the call goes on, or rings on, on the phone). */
  readonly onClose: () => void;
}

/** The call window's one screen for `view` (`null` or closed: nothing). */
export function CallWindowScreen({
  view,
  who,
  name,
  phone,
  on,
}: {
  readonly view: CallWindowView | null;
  /** The caller's name or number, when known. */
  readonly who: string | null;
  /** The caller's contact name, when known (for the initials). */
  readonly name: string | null;
  readonly phone: string;
  readonly on: CallWindowIntents;
}) {
  switch (view?.kind) {
    case 'ringing':
      return (
        <IncomingCall
          who={who ?? view.caller.number}
          name={view.caller.name}
          phone={phone}
          on={{ onAnswer: on.onAnswer, onDecline: on.onDecline, onAnswerOnPhone: on.onClose }}
        />
      );
    case 'active':
      return <InCall who={who} name={name} phone={phone} view={view} on={on} />;
    case 'missed':
      return (
        <MissedCall
          who={who ?? view.caller.number}
          name={view.caller.name}
          phone={phone}
          on={{ onCallBack: on.onCallBack, onClose: on.onClose }}
        />
      );
    case 'closed':
    case undefined:
      return null;
  }
}
