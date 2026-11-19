import { useEffect, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { EcosystemToasts } from '../app/shell/EcosystemToasts';
import { usePort } from '../engine/port';
import type { Feature, LinkedDeviceView } from '../engine/types';
import { current } from '../flows/remote';
import { PHASE2_FEATURES, type Phase2Feature } from '../screens/devices/model';
import { Toast, Window } from '../ui';
import { PhoneDevice, type PhoneScreen } from './home/PhoneDevice';
import { PhoneHome } from './home/PhoneHome';
import { shareChanges, type OpenedIntent } from './model';
import { ChooseShare } from './pairing/ChooseShare';
import { PhonePairing } from './pairing/PhonePairing';
import { Welcome } from './pairing/Welcome';
import { PhoneActivity } from './PhoneActivity';
import { Calls } from './phone/Calls';
import { Messages } from './phone/Messages';
import { FEATURE_PERMS, permsFor } from './phone/model';
import { NotifyApps } from './phone/NotifyApps';
import { PermStep } from './phone/PermStep';
import { usePermGate } from './phone/usePermGate';
import { PhoneOverlay, type PhoneOverlayModel } from './PhoneOverlay';
import { PhoneSettings } from './PhoneSettings';
import { PhoneTabs, type PhoneTab } from './PhoneTabs';
import { UpdatedToast } from './update/UpdatedToast';
import { UpdatePage } from './update/UpdatePage';
import { useUpdated } from './update/useUpdated';
import { useOpened } from './useOpened';
import { useSendNote, NOTE_KEY } from './useSendNote';
import './mobile.css';

/** Which phone screen shows. */
type PhoneView =
  | { readonly kind: PhoneTab }
  | { readonly kind: 'pair' }
  | { readonly kind: 'update' }
  | { readonly kind: 'choose'; readonly device: LinkedDeviceView }
  | { readonly kind: 'device'; readonly id: string }
  /** A phone feature's own screen, opened from computer `id`'s page. */
  | { readonly kind: PhoneScreen; readonly id: string };

/**
 * The phone app (`about.os === 'android'`): the same engine, controllers and
 * events as the desktop, laid out for a phone. Welcome until a computer is
 * linked, then Home, Activity and Settings.
 */
export function PhoneApp() {
  const { t } = useTranslation();
  const { linked, update, settings } = useControllers();
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
  const updated = useUpdated(usePort());
  const offer = current(update.check);
  const offered = offer?.status === 'available' ? offer.version : null;
  // Once settings load, look for an update if the user lets Carrova (never downloads by itself).
  const autoCheck = settings.settings?.auto_check_app;
  const runCheck = update.runCheck;
  const autoChecked = useRef(false);
  useEffect(() => {
    if (autoCheck !== true || autoChecked.current) return;
    autoChecked.current = true;
    runCheck();
  }, [autoCheck, runCheck]);
  // Android permissions, asked when a feature needing them turns on.
  const gate = usePermGate();
  const turnOn = (feature: Feature) => {
    gate.ask(FEATURE_PERMS[feature as Phase2Feature] ?? []);
  };
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
  const openUpdate = () => {
    setView({ kind: 'update' });
  };
  const page = ((): ReactNode => {
    switch (view.kind) {
      case 'update':
        return (
          <UpdatePage
            onBack={() => {
              setView({ kind: 'home' });
            }}
          />
        );
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
            onTurnOn={turnOn}
            onDone={(choices) => {
              for (const [feature, state] of shareChanges(view.device, choices))
                setFeature(view.device.id, feature, state);
              // Preselected features never turned on here: ask for theirs now.
              gate.ask(permsFor(PHASE2_FEATURES.filter((f) => choices[f] === 'on')));
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
            onScreen={(kind) => {
              setView({ kind, id: device.id });
            }}
            on={{
              onFeature: (feature, state) => {
                setFeature(device.id, feature, state);
                if (state === 'on') turnOn(feature);
              },
              // Computers cannot ring.
              onRing: () => undefined,
              onStopRing: () => undefined,
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
      case 'notifications':
      case 'messages':
      case 'calls': {
        const device = linked.devices.find((d) => d.id === view.id);
        const name = device?.name ?? '';
        const back = () => {
          setView({ kind: 'device', id: view.id });
        };
        const allow = () => {
          gate.ask(FEATURE_PERMS[view.kind] ?? []);
        };
        if (view.kind === 'notifications')
          return <NotifyApps name={name} onBack={back} onAllow={allow} />;
        if (view.kind === 'calls') {
          const on =
            device?.features.some((f) => f.feature === 'calls' && f.effective !== 'off') ?? false;
          return <Calls name={name} on={on} onBack={back} onAllow={allow} />;
        }
        return (
          <Messages
            name={name}
            onBack={back}
            onAllow={allow}
            onNotifications={() => {
              setView({ kind: 'notifications', id: view.id });
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
              onUpdate: openUpdate,
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
            update={offered}
            on={{
              onAdd: pair,
              onOpen: open,
              onUpdate: openUpdate,
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
          {gate.asking !== null && (
            <PermStep
              perm={gate.asking.perm}
              step={gate.asking.step}
              refused={gate.refused}
              onAllow={gate.allow}
              onOpen={gate.open}
              onBack={gate.close}
            />
          )}
          {/* Kept mounted under a permission step, so Choose what to share keeps its choices. */}
          <div className="m-keep" hidden={gate.asking !== null}>
            {page}
          </div>
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
        {updated.version !== null && (
          <UpdatedToast version={updated.version} onDismiss={updated.dismiss} />
        )}
        {note !== null && (
          <Toast icon={note.kind === 'actionFailed' ? 'warn' : 'check'} onDismiss={dismiss}>
            {t(NOTE_KEY[note.kind], { name: note.name })}
          </Toast>
        )}
      </Window>
    </div>
  );
}
