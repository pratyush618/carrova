import { useEffect, useState } from 'react';
import { useControllers } from '../app/controllers/Controllers';
import type { LinkedDeviceView } from '../engine/types';
import { Window } from '../ui';
import { PhoneHome } from './home/PhoneHome';
import { shareChanges } from './model';
import { ChooseShare } from './pairing/ChooseShare';
import { PhonePairing } from './pairing/PhonePairing';
import { Welcome } from './pairing/Welcome';
import './mobile.css';

/** Which phone screen shows. */
type PhoneView =
  | { readonly kind: 'home' }
  | { readonly kind: 'pair' }
  | { readonly kind: 'choose'; readonly device: LinkedDeviceView };

/**
 * The phone app (`about.os === 'android'`): the same engine, controllers and
 * events as the desktop, laid out for a phone. Welcome until a computer is
 * linked, then Home.
 */
export function PhoneApp() {
  const { linked } = useControllers();
  const [view, setView] = useState<PhoneView>({ kind: 'home' });
  // Welcome or Home only once the list is known, so neither flashes.
  const [loaded, setLoaded] = useState(false);
  const { reload, setFeature } = linked;
  useEffect(() => {
    void reload().then(() => {
      setLoaded(true);
    });
  }, [reload]);

  const home = () => {
    setView({ kind: 'home' });
  };
  const pair = () => {
    setView({ kind: 'pair' });
  };
  const page = (() => {
    switch (view.kind) {
      case 'pair':
        return (
          <PhonePairing
            onClose={home}
            onChoose={(device) => {
              if (device === null) home();
              else setView({ kind: 'choose', device });
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
              home();
            }}
          />
        );
      case 'home':
        if (!loaded) return null;
        return linked.devices.length === 0 ? (
          <Welcome onAdd={pair} />
        ) : (
          <PhoneHome devices={linked.devices} onAdd={pair} />
        );
    }
  })();
  return (
    <div className="m-app">
      <Window>{page}</Window>
    </div>
  );
}
