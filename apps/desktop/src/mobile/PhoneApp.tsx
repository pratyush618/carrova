import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { EcosystemToasts } from '../app/shell/EcosystemToasts';
import type { LinkedDeviceView } from '../engine/types';
import { Toast, Window } from '../ui';
import { PhoneDevice } from './home/PhoneDevice';
import { PhoneHome } from './home/PhoneHome';
import { shareChanges, type OpenedIntent } from './model';
import { ChooseShare } from './pairing/ChooseShare';
import { PhonePairing } from './pairing/PhonePairing';
import { Welcome } from './pairing/Welcome';
import { PhoneActivity } from './PhoneActivity';
import { PhoneOverlay, type PhoneOverlayModel } from './PhoneOverlay';
import { PhoneSettings } from './PhoneSettings';
import { PhoneTabs, type PhoneTab } from './PhoneTabs';
import { useOpened } from './useOpened';
import { useSendNote, NOTE_KEY } from './useSendNote';
import './mobile.css';

/** Which phone screen shows. */
type PhoneView =
  | { readonly kind: PhoneTab }
  | { readonly kind: 'pair' }
  | { readonly kind: 'choose'; readonly device: LinkedDeviceView }
  | { readonly kind: 'device'; readonly id: string };

/**
 * The phone app (`about.os === 'android'`): the same engine, controllers and
 * events as the desktop, laid out for a phone. Welcome until a computer is
 * linked, then Home, Activity and Settings.
 */
export function PhoneApp() {
  const { t } = useTranslation();
  const { linked } = useControllers();
  const [view, setView] = useState<PhoneView>({ kind: 'home' });
  // What the user opened here (a send, Forget), then the links the app was
  // opened with, in turn: one never replaces another.
  const [overlay, setOverlay] = useState<PhoneOverlayModel | null>(null);
  const [opened, setOpened] = useState<readonly OpenedIntent[]>([]);
  // How many opened links were dismissed: keys each one's sheet afresh.
  const [taken, setTaken] = useState(0);
  const { note, sendTo, dismiss } = useSendNote();
  // Welcome or Home only once the list is known, so neither flashes.
  const [loaded, setLoaded] = useState(false);
  const { reload, setFeature, sendClipboard } = linked;
  useOpened((intent) => {
    setOpened((queue) => [...queue, intent]);
  });
  useEffect(() => {
    void reload().then(() => {
      setLoaded(true);
    });
  }, [reload]);

  const shown = overlay ?? opened[0] ?? null;
  const close = () => {
    if (overlay !== null) {
      setOverlay(null);
    } else {
      setOpened((queue) => queue.slice(1));
      setTaken((n) => n + 1);
    }
  };
  const pair = () => {
    setView({ kind: 'pair' });
  };
  const open = (id: string) => {
    setView({ kind: 'device', id });
  };
  const page = ((): ReactNode => {
    switch (view.kind) {
      case 'pair':
        return (
          <PhonePairing
            onClose={() => {
              setView({ kind: 'home' });
            }}
            onChoose={(device) => {
              setView(device === null ? { kind: 'home' } : { kind: 'choose', device });
            }}
          />
        );
      case 'choose':
        return (
          <ChooseShare
            device={view.device}
            onDone={(choices) => {
              for (const [feature, state] of shareChanges(view.device, choices))
                setFeature(view.device.id, feature, state);
              setView({ kind: 'home' });
            }}
          />
        );
      case 'device': {
        const device = linked.devices.find((d) => d.id === view.id);
        if (device === undefined) return null;
        return (
          <PhoneDevice
            device={device}
            failed={linked.failed}
            onBack={() => {
              setView({ kind: 'home' });
            }}
            on={{
              onFeature: (feature, state) => {
                setFeature(device.id, feature, state);
              },
              // Computers cannot ring.
              onRing: () => undefined,
              onSendClipboard: () => {
                sendTo(device, sendClipboard, 'clipSent');
              },
              onForget: () => {
                setOverlay({ kind: 'forget', id: device.id, name: device.name });
              },
            }}
          />
        );
      }
      case 'activity':
        return <PhoneActivity />;
      case 'settings':
        return (
          <PhoneSettings
            on={{
              onOpen: open,
              onAdd: pair,
              onActivity: () => {
                setView({ kind: 'activity' });
              },
            }}
          />
        );
      case 'home':
        if (!loaded) return null;
        return linked.devices.length === 0 ? (
          <Welcome onAdd={pair} />
        ) : (
          <PhoneHome
            devices={linked.devices}
            on={{
              onAdd: pair,
              onOpen: open,
              onSendClipboard: () => {
                setOverlay({ kind: 'clipboard' });
              },
            }}
          />
        );
    }
  })();

  // The tabs show on the three places, once a computer is linked.
  const tab = view.kind === 'home' || view.kind === 'activity' || view.kind === 'settings';
  return (
    <div className="m-app">
      <Window
        overlays={
          shown === null
            ? []
            : [
                <PhoneOverlay
                  key={overlay === null ? `opened-${String(taken)}` : overlay.kind}
                  overlay={shown}
                  sendTo={sendTo}
                  onClose={close}
                  onForgotten={() => {
                    setView({ kind: 'home' });
                  }}
                />,
              ]
        }
      >
        <div className="m-frame">
          {page}
          {tab && linked.devices.length > 0 && (
            <PhoneTabs
              current={view.kind}
              onSelect={(kind) => {
                setView({ kind });
              }}
            />
          )}
        </div>
        <EcosystemToasts />
        {note !== null && (
          <Toast icon={note.kind === 'actionFailed' ? 'warn' : 'check'} onDismiss={dismiss}>
            {t(NOTE_KEY[note.kind], { name: note.name })}
          </Toast>
        )}
      </Window>
    </div>
  );
}
