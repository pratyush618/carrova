import { useEffect, useRef, useState } from 'react';
import type { CallWindowView } from '../../../desktop/src/bindings/CallWindowView';
import type { FeatureRowView } from '../../../desktop/src/bindings/FeatureRowView';
import type { LinkedDeviceView } from '../../../desktop/src/bindings/LinkedDeviceView';
import type { NoteView } from '../../../desktop/src/bindings/NoteView';
import { PhoneHome } from '../../../desktop/src/mobile/home/PhoneHome';
import { PhoneTabs } from '../../../desktop/src/mobile/PhoneTabs';
import { CallWindowScreen } from '../../../desktop/src/screens/phone/calls/CallWindowScreen';
import { NotePopup } from '../../../desktop/src/screens/phone/notes/NotePopup';
import { Button, Icon, Window } from '../../../desktop/src/ui';
import { usePointerDepth } from '../usePointerDepth';

// The laptop's screen: the app's own page for the linked Pixel 8, in each theme.
const SCREENS = import.meta.glob<string>('../assets/shots/device.*-900.webp', {
  eager: true,
  import: 'default',
});
const screenFor = (scheme: 'light' | 'dark') =>
  Object.entries(SCREENS).find(([path]) => path.includes(`device.${scheme}-`))?.[1];

const ignore = (): void => undefined;
const PHONE = 'Pixel 8';
const COMPUTER = 'Desk PC';
const CALLER = { number: '+1 555 0101', name: 'Sam Rivera' };

const row = (
  feature: FeatureRowView['feature'],
  state: FeatureRowView['local'],
): FeatureRowView => ({
  feature,
  group: feature === 'files' || feature === 'handoff' ? 'ask' : 'quiet',
  local: state,
  remote: state,
  effective: state,
});

/** The computer as the phone sees it, linked with the phone's defaults. */
const DESK_PC: LinkedDeviceView = {
  id: 'd2',
  name: COMPUTER,
  os: 'linux',
  fingerprint: 'B04E·88D1',
  state: 'linked',
  features: [
    row('clipboard', 'on'),
    row('ring', 'on'),
    row('notifications', 'on'),
    row('messages', 'on'),
    row('calls', 'on'),
    row('files', 'ask'),
  ],
  missing: [],
};

const TEXT: NoteView = {
  key: 'n1',
  app_id: 'org.chat',
  app_name: 'Messages',
  title: 'Sam Rivera',
  text: 'Are you still coming tonight?',
  when_ms: Date.now(),
  can_reply: true,
  icon: null,
  hidden: null,
};

/** What Android itself shows at the top of the phone (drawn here, not Carrova's UI). */
interface HeadsUp {
  readonly app: string;
  readonly title: string;
  readonly text: string;
}

type Call = 'ringing' | 'active' | null;

const clock = (s: number) =>
  `${String(Math.floor(s / 60)).padStart(2, '0')}:${String(s % 60).padStart(2, '0')}`;

/** Whole seconds since `since` (ms), ticking while there is one. */
function useSecondsSince(since: number | null): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (since === null) return undefined;
    const timer = window.setInterval(() => {
      setNow(Date.now());
    }, 500);
    return () => {
      window.clearInterval(timer);
    };
  }, [since]);
  return since === null ? 0 : Math.max(0, Math.floor((now - since) / 1000));
}

const TEXT_HEADS_UP: HeadsUp = { app: 'Messages', title: TEXT.title, text: TEXT.text };

function StatusBar({ time }: { readonly time: string }) {
  return (
    <div className="phone-status" aria-hidden="true">
      <span>{time}</span>
      <span className="phone-status__icons">
        <svg viewBox="0 0 16 12">
          <path d="M1 11h2V8H1zm4 0h2V6H5zm4 0h2V3H9zm4 0h2V0h-2z" />
        </svg>
        <svg viewBox="0 0 16 12">
          <path d="M8 11.5 15.6 3.8A11 11 0 0 0 .4 3.8z" />
        </svg>
        <svg viewBox="0 0 24 12">
          <rect x="0.5" y="0.5" width="20" height="11" rx="3" fill="none" stroke="currentColor" />
          <rect x="2.5" y="2.5" width="13" height="7" rx="1.5" />
          <rect x="21.5" y="4" width="2" height="4" rx="1" />
        </svg>
      </span>
    </div>
  );
}

/**
 * A linked phone next to the computer. The computer side is Carrova's own
 * pop-up and call window; the phone runs Carrova's phone Home, under the
 * notifications and call screen Android itself would show.
 */
export function PhoneScene() {
  const [headsUp, setHeadsUp] = useState<HeadsUp | null>(null);
  const [note, setNote] = useState(false);
  const [call, setCall] = useState<Call>(null);
  const [answeredAt, setAnsweredAt] = useState<number | null>(null);
  const [audio, setAudio] = useState<'laptop' | 'phone'>('laptop');
  const [muted, setMuted] = useState(false);
  const [ringing, setRinging] = useState(false);
  const [toast, setToast] = useState(false);
  const scene = useRef<HTMLDivElement>(null);
  usePointerDepth(scene);
  const seconds = useSecondsSince(call === 'active' ? answeredAt : null);
  const answer = (side: 'laptop' | 'phone') => {
    setAudio(side);
    setAnsweredAt(Date.now());
    setCall('active');
  };

  // Once the visitor has tried something, the opening text no longer arrives.
  const touched = useRef(false);
  const clear = () => {
    touched.current = true;
    setHeadsUp(null);
    setNote(false);
    setCall(null);
    setRinging(false);
    setToast(false);
  };

  const textArrives = () => {
    clear();
    setHeadsUp(TEXT_HEADS_UP);
    window.setTimeout(() => {
      setNote(true);
    }, 700);
  };

  // A text arrives on its own the first time the scene shows.
  useEffect(() => {
    const arrive = window.setTimeout(() => {
      if (!touched.current) setHeadsUp(TEXT_HEADS_UP);
    }, 900);
    const pop = window.setTimeout(() => {
      if (!touched.current) setNote(true);
    }, 1600);
    return () => {
      window.clearTimeout(arrive);
      window.clearTimeout(pop);
    };
  }, []);

  // A heads-up notification slides away by itself, as Android's do.
  useEffect(() => {
    if (headsUp === null || note) return undefined;
    const timer = window.setTimeout(() => {
      setHeadsUp(null);
    }, 3200);
    return () => {
      window.clearTimeout(timer);
    };
  }, [headsUp, note]);

  useEffect(() => {
    if (!toast) return undefined;
    const timer = window.setTimeout(() => {
      setToast(false);
    }, 2600);
    return () => {
      window.clearTimeout(timer);
    };
  }, [toast]);

  const view: CallWindowView | null =
    call === 'ringing'
      ? { kind: 'ringing', device: 'p8', caller: CALLER, ring_here: true }
      : call === 'active'
        ? {
            kind: 'active',
            device: 'p8',
            timer: clock(seconds),
            muted,
            audio,
            keypad: audio === 'laptop',
          }
        : null;

  return (
    <div className="scene-phone" ref={scene}>
      <div className="desk" aria-label="The computer">
        <p className="desk__hint">
          <Icon name="laptop" /> {COMPUTER}
        </p>
        <div className="desk__screen" aria-hidden="true">
          <img data-scheme="light" src={screenFor('light')} alt="" />
          <img data-scheme="dark" src={screenFor('dark')} alt="" />
        </div>
        <div className="desk__try" role="group" aria-label="Try it">
          <Button size="sm" icon="msg" onClick={textArrives}>
            A text arrives
          </Button>
          <Button
            size="sm"
            icon="call"
            onClick={() => {
              clear();
              setMuted(false);
              setCall('ringing');
            }}
          >
            A call comes in
          </Button>
          <Button
            size="sm"
            icon="ring"
            onClick={() => {
              clear();
              setRinging(true);
            }}
          >
            Ring the phone
          </Button>
          <Button
            size="sm"
            icon="copy"
            onClick={() => {
              clear();
              setToast(true);
            }}
          >
            Copy on the computer
          </Button>
        </div>
      </div>

      <div className="layer-float desk__popups">
        {note && (
          <div className="popup popup--in">
            <NotePopup
              model={{ kind: 'note', phone: PHONE, note: TEXT }}
              on={{
                onReply: (text) => {
                  setNote(false);
                  setHeadsUp({ app: 'Messages', title: 'You replied from Desk PC', text });
                },
                onDismiss: clear,
                onOpen: () => {
                  setNote(false);
                },
                onClose: () => {
                  setNote(false);
                },
              }}
            />
          </div>
        )}
        {view && (
          <div className="popup popup--in popup--call">
            <Window overlays={[]}>
              <CallWindowScreen
                view={view}
                who={CALLER.name}
                name={CALLER.name}
                phone={PHONE}
                sims={[]}
                on={{
                  onAnswer: () => {
                    answer('laptop');
                  },
                  onDecline: clear,
                  onMute: setMuted,
                  onTone: ignore,
                  onAudioTo: setAudio,
                  onEnd: clear,
                  onCallBack: ignore,
                  onClose: clear,
                }}
              />
            </Window>
          </div>
        )}
      </div>

      <div className="phone" aria-label={PHONE}>
        <div className="phone__screen">
          <StatusBar time="9:41" />
          <div className="phone__app">
            <div className="m-app">
              <Window overlays={[]}>
                <div className="m-frame">
                  <PhoneHome
                    devices={[DESK_PC]}
                    update={null}
                    on={{
                      onAdd: ignore,
                      onOpen: ignore,
                      onSendClipboard: () => {
                        setToast(false);
                        setHeadsUp({
                          app: 'Carrova',
                          title: 'Clipboard sent',
                          text: `On ${COMPUTER} now. Paste it there.`,
                        });
                      },
                      onUpdate: ignore,
                    }}
                  />
                  <PhoneTabs current="home" onSelect={ignore} />
                </div>
              </Window>
            </div>
          </div>

          {call === 'active' && (
            <p className="phone-pill">
              <Icon name="call" /> {clock(seconds)} ·{' '}
              {audio === 'laptop' ? `audio on ${COMPUTER}` : 'on the phone'}
            </p>
          )}

          {headsUp && (
            <div className="heads-up" role="status">
              <p className="heads-up__app">{headsUp.app} · now</p>
              <p className="heads-up__title">{headsUp.title}</p>
              <p>{headsUp.text}</p>
            </div>
          )}

          {toast && (
            <div className="heads-up" role="status">
              <p className="heads-up__app">Carrova · now</p>
              <p className="heads-up__title">Clipboard from {COMPUTER}</p>
              <p>“Meet at 7, gate B” is ready to paste.</p>
            </div>
          )}

          {call === 'ringing' && (
            <div className="phone-call">
              <p className="phone-call__who">{CALLER.name}</p>
              <p>Mobile {CALLER.number}</p>
              <span className="phone-call__avatar" aria-hidden="true">
                SR
              </span>
              <div className="phone-call__actions">
                <button
                  type="button"
                  className="round round--no"
                  onClick={clear}
                  aria-label="Decline"
                >
                  <Icon name="callEnd" />
                </button>
                <button
                  type="button"
                  className="round round--yes"
                  onClick={() => {
                    answer('phone');
                  }}
                  aria-label="Answer on the phone"
                >
                  <Icon name="call" />
                </button>
              </div>
            </div>
          )}

          {ringing && (
            <div className="phone-ring">
              <span className="phone-ring__bell" aria-hidden="true">
                <Icon name="ring" />
              </span>
              <p className="phone-call__who">Found me?</p>
              <p>{COMPUTER} is ringing this phone, even on silent.</p>
              <button
                type="button"
                className="phone-ring__stop"
                onClick={() => {
                  setRinging(false);
                }}
              >
                Stop ringing
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
