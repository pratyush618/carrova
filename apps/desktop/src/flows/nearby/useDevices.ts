import { useCallback, useEffect, useMemo } from 'react';
import { useRemembered } from '../../app/state/EngineState';
import type {
  CarrovaPort,
  ForgetDevices,
  KnownDeviceView,
  NearbyDeviceView,
  VisibilityChoice,
  VisibilityView,
} from '../../engine/types';
import { current } from '../remote';
import { useRemote } from '../useRemote';

export interface DevicesController {
  /** Who can find this computer: the one value the Nearby pill and Settings share. `null` until loaded. */
  readonly visibility: VisibilityView | null;
  /** Remembered devices (empty until loaded). */
  readonly known: readonly KnownDeviceView[];
  /** Look for devices nearby (one browse). */
  readonly browse: () => Promise<NearbyDeviceView[]>;
  readonly setVisibility: (choice: VisibilityChoice) => void;
  readonly rename: (device: string, name: string) => void;
  readonly forget: (which: ForgetDevices) => void;
  readonly resetIdentity: () => void;
  /** Ask the engine again (the tray or the CLI may have changed things). */
  readonly refresh: () => void;
  /** Wake a sleeping known device; rejects with the bridge error. */
  readonly wake: (device: string) => Promise<void>;
}

/** Visibility and remembered devices: loaded on mount, reloaded after every change. */
export function useDevices(port: CarrovaPort): DevicesController {
  const loaded = useRemote(useCallback(() => port.visibility(), [port]));
  const knownState = useRemote(useCallback(() => port.knownDevices(), [port]));
  const visibility = current(loaded.state);
  const reloadKnown = knownState.run;
  const remembered = useRemembered();

  useEffect(() => {
    if (remembered !== null) void reloadKnown();
  }, [remembered, reloadKnown]);

  const { run: reloadVisibility } = loaded;
  const refreshVisibility = useCallback(() => void reloadVisibility(), [reloadVisibility]);
  const refresh = useCallback(() => {
    refreshVisibility();
    void reloadKnown();
  }, [refreshVisibility, reloadKnown]);

  // "Everyone" turns back into "Known devices" by itself: ask again when it does.
  const until = visibility?.until ?? null;
  useEffect(() => {
    if (until === null) return;
    const timer = setTimeout(refreshVisibility, Math.max(0, until * 1000 - Date.now()));
    return () => {
      clearTimeout(timer);
    };
  }, [until, refreshVisibility]);

  const setVisibility = useCallback(
    (choice: VisibilityChoice) => {
      void port
        .setVisibility(choice)
        .catch(() => undefined)
        .then(refreshVisibility);
    },
    [port, refreshVisibility],
  );
  const after = useCallback(
    (change: Promise<void>) => {
      // A refused change leaves the list as the engine has it.
      void change.catch(() => undefined).then(() => reloadKnown());
    },
    [reloadKnown],
  );
  const rename = useCallback(
    (device: string, name: string) => {
      after(port.renameDevice(device, name));
    },
    [port, after],
  );
  const forget = useCallback(
    (which: ForgetDevices) => {
      after(port.forgetDevice(which));
    },
    [port, after],
  );
  const resetIdentity = useCallback(() => {
    after(port.resetIdentity());
  }, [port, after]);
  const browse = useCallback(() => port.nearby(), [port]);
  const wake = useCallback((device: string) => port.wakeDevice(device), [port]);

  const known = current(knownState.state);
  return useMemo(
    () => ({
      visibility,
      known: known ?? [],
      browse,
      setVisibility,
      rename,
      forget,
      resetIdentity,
      refresh,
      wake,
    }),
    [visibility, known, browse, setVisibility, rename, forget, resetIdentity, refresh, wake],
  );
}
