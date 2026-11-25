import { useState } from 'react';
import { NearbyScreen } from '../../../desktop/src/screens/nearby/NearbyScreen';
import { nearbyMap } from '../../../desktop/src/screens/nearby/fixtures';
import type { NearbyView } from '../../../desktop/src/screens/nearby/model';
import { AppWindow } from './AppWindow';

const ignore = (): void => undefined;

/** Nearby as the app shows it: open a device's card, or switch to the list. */
export function NearbyScene() {
  const [view, setView] = useState<NearbyView>('map');
  const [selected, setSelected] = useState<string | null>(null);
  return (
    <div className="live-nearby">
      <AppWindow view={{ kind: 'hub', page: 'nearby' }}>
        <NearbyScreen
          model={{ ...nearbyMap, view, selected }}
          on={{
            onView: setView,
            onSelect: setSelected,
            onVisibility: ignore,
            onSettings: ignore,
            onSendWithCode: ignore,
            onWake: ignore,
            onForget: ignore,
            onDismissWake: ignore,
            onOpen: ignore,
            onAddDevice: ignore,
            onLinkComputer: ignore,
          }}
        />
      </AppWindow>
    </div>
  );
}
