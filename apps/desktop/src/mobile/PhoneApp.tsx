import { useEffect, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { EcosystemToasts } from '../app/shell/EcosystemToasts';
import type { LinkedDeviceView } from '../engine/types';
import { ForgetDialog } from '../screens/devices/ForgetDialog';
import { Toast, Window } from '../ui';
import { PhoneDevice } from './home/PhoneDevice';
import { PhoneHome } from './home/PhoneHome';
import { PickComputerSheet } from './home/PickComputerSheet';
import { readyFor, shareChanges } from './model';
import { ChooseShare } from './pairing/ChooseShare';
import { PhonePairing } from './pairing/PhonePairing';
import { Welcome } from './pairing/Welcome';
import { PhoneActivity } from './PhoneActivity';
import { PhoneSettings } from './PhoneSettings';
import { PhoneTabs, type PhoneTab } from './PhoneTabs';
import './mobile.css';

/** Which phone screen shows. */
type PhoneView =
  | { readonly kind: PhoneTab }
  | { readonly kind: 'pair' }
  | { readonly kind: 'choose'; readonly device: LinkedDeviceView }
  | { readonly kind: 'device'; readonly id: string };

/** What sits over the screen. */
type PhoneOverlay =
  | { readonly kind: 'clipboard' }
  | { readonly kind: 'forget'; readonly id: string; readonly name: string };

/** What the last action said: sent, or the computer didn't answer. */
interface Note {
  readonly kind: 'clipSent' | 'actionFailed';
  readonly name: string;
}

/**
 * The phone app (`about.os === 'android'`): the same engine, controllers and
 * events as the desktop, laid out for a phone. Welcome until a computer is
 * linked, then Home, Activity and Settings.
 */
export function PhoneApp() {
  const { t } = useTranslation();
  const { linked, devices: nearby } = useControllers();
  const [view, setView] = useState<PhoneView>({ kind: 'home' });
  const [overlay, setOverlay] = useState<PhoneOverlay | null>(null);
  const [note, setNote] = useState<Note | null>(null);
  // Welcome or Home only once the list is known, so neither flashes.
  const [loaded, setLoaded] = useState(false);
  const { reload, setFeature, sendClipboard } = linked;
  useEffect(() => {
    void reload().then(() => {
      setLoaded(true);
    });
  }, [reload]);

  const go = (next: PhoneView) => {
    setView(next);
  };
  const close = () => {
    setOverlay(null);
  };
  const pair = () => {
    go({ kind: 'pair' });
  };
  const open = (id: string) => {
    go({ kind: 'device', id });
  };
  const sendClip = (device: LinkedDeviceView) => {
    sendClipboard(device.id).then(
      () => {
        setNote({ kind: 'clipSent', name: device.name });
      },
      () => {
        setNote({ kind: 'actionFailed', name: device.name });
      },
    );
  };

  const page = ((): ReactNode => {
    switch (view.kind) {
      case 'pair':
        return (
          <PhonePairing
            onClose={() => {
              go({ kind: 'home' });
            }}
            onChoose={(device) => {
              go(device === null ? { kind: 'home' } : { kind: 'choose', device });
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
              go({ kind: 'home' });
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
              go({ kind: 'home' });
            }}
            on={{
              onFeature: (feature, state) => {
                setFeature(device.id, feature, state);
              },
              // Computers cannot ring.
              onRing: () => undefined,
              onSendClipboard: () => {
                sendClip(device);
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
                go({ kind: 'activity' });
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

  const over = ((): ReactNode => {
    switch (overlay?.kind) {
      case undefined:
        return null;
      case 'clipboard':
        return (
          <PickComputerSheet
            title={t('mobile.home.sendClipboard')}
            devices={readyFor(linked.devices, 'clipboard')}
            onClose={close}
            onSend={(id) => {
              close();
              const device = linked.devices.find((d) => d.id === id);
              if (device !== undefined) sendClip(device);
            }}
          />
        );
      case 'forget': {
        const { id, name } = overlay;
        return (
          <ForgetDialog
            name={name}
            on={{
              onCancel: close,
              onForget: () => {
                close();
                void nearby.forget({ device: id }).then(() => reload());
                go({ kind: 'home' });
              },
            }}
          />
        );
      }
    }
  })();

  // The tabs show on the three places, once a computer is linked.
  const tab = view.kind === 'home' || view.kind === 'activity' || view.kind === 'settings';
  return (
    <div className="m-app">
      <Window overlays={over === null ? [] : [over]}>
        <div className="m-frame">
          {page}
          {tab && linked.devices.length > 0 && (
            <PhoneTabs
              current={view.kind}
              onSelect={(kind) => {
                go({ kind });
              }}
            />
          )}
        </div>
        <EcosystemToasts />
        {note !== null && (
          <Toast
            icon={note.kind === 'clipSent' ? 'check' : 'warn'}
            onDismiss={() => {
              setNote(null);
            }}
          >
            {t(`ecosystem.device.${note.kind}`, { name: note.name })}
          </Toast>
        )}
      </Window>
    </div>
  );
}
