import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRemembered } from '../../app/state/EngineState';
import type {
  CarrovaPort,
  ForgetDevices,
  KnownDeviceView,
  NearbyDeviceView,
  VisibilityChoice,
  VisibilityView,
} from '../../engine/types';
import type { DeviceChange } from '../../screens/nearby/model';

export interface DevicesController {
  /** Who can find this computer: the one value the Nearby pill and Settings share. `null` until loaded. */
  readonly visibility: VisibilityView | null;
  /** Remembered devices (empty until loaded). */
  readonly known: readonly KnownDeviceView[];
  /** The last change the engine refused, until the next change. */
  readonly failed: DeviceChange | null;
  /** Look for devices nearby (one browse). */
  readonly browse: () => Promise<NearbyDeviceView[]>;
  readonly setVisibility: (choice: VisibilityChoice) => void;
  readonly rename: (device: string, name: string) => void;
  readonly forget: (which: ForgetDevices) => void;
  readonly resetIdentity: () => void;
  /** Wake a sleeping known device; rejects with the bridge error. */
  readonly wake: (device: string) => Promise<void>;
}

/**
 * Visibility and remembered devices: loaded on mount, when the window comes
 * back (the tray or the CLI may have changed them), when a move remembers a
 * device, when "everyone" runs out, and after every change made here. An
 * answer that started before a newer load or change is dropped, so a slow
 * refresh never undoes the user's own change.
 */
export function useDevices(port: CarrovaPort): DevicesController {
  const [visibility, setShown] = useState<VisibilityView | null>(null);
  const [known, setKnown] = useState<readonly KnownDeviceView[]>([]);
  const [failed, setFailed] = useState<DeviceChange | null>(null);
  /** Bumped by every load and change: only the latest load's answer lands. */
  const latest = useRef(0);
  /** Changes still on their way; each loads again once done. */
  const changing = useRef(0);
  /** Bumped by every change: only the latest change's failure shows. */
  const lastChange = useRef(0);
  const remembered = useRemembered();

  const load = useCallback(() => {
    const ticket = ++latest.current;
    void Promise.allSettled([port.visibility(), port.knownDevices()]).then(([shown, list]) => {
      if (ticket !== latest.current || changing.current > 0) return;
      if (shown.status === 'fulfilled') setShown(shown.value);
      if (list.status === 'fulfilled') setKnown(list.value);
    });
  }, [port]);

  useEffect(() => {
    const onShow = () => {
      if (document.visibilityState === 'visible') load();
    };
    load();
    window.addEventListener('focus', load);
    document.addEventListener('visibilitychange', onShow);
    return () => {
      window.removeEventListener('focus', load);
      document.removeEventListener('visibilitychange', onShow);
    };
  }, [load]);

  useEffect(() => {
    if (remembered !== null) load();
  }, [remembered, load]);

  // "Everyone" turns back into "Known devices" by itself: ask again when it does.
  const until = visibility?.until ?? null;
  useEffect(() => {
    if (until === null) return;
    const timer = setTimeout(load, Math.max(0, until * 1000 - Date.now()));
    return () => {
      clearTimeout(timer);
    };
  }, [until, load]);

  const change = useCallback(
    (what: DeviceChange, run: () => Promise<unknown>): Promise<boolean> => {
      latest.current++;
      changing.current++;
      const mine = ++lastChange.current;
      setFailed(null);
      return run()
        .then(
          () => true,
          () => {
            if (mine === lastChange.current) setFailed(what);
            return false;
          },
        )
        .finally(() => {
          changing.current--;
          load();
        });
    },
    [load],
  );
  const setVisibility = useCallback(
    (choice: VisibilityChoice) => {
      void change('visibility', () => port.setVisibility(choice));
    },
    [port, change],
  );
  const rename = useCallback(
    (device: string, name: string) => {
      void change('rename', () => port.renameDevice(device, name));
    },
    [port, change],
  );
  const forget = useCallback(
    (which: ForgetDevices) => {
      void change('forget', () => port.forgetDevice(which));
    },
    [port, change],
  );
  const resetIdentity = useCallback(() => {
    void change('reset', () => port.resetIdentity());
  }, [port, change]);
  const browse = useCallback(() => port.nearby(), [port]);
  const wake = useCallback((device: string) => port.wakeDevice(device), [port]);

  return useMemo(
    () => ({
      visibility,
      known,
      failed,
      browse,
      setVisibility,
      rename,
      forget,
      resetIdentity,
      wake,
    }),
    [visibility, known, failed, browse, setVisibility, rename, forget, resetIdentity, wake],
  );
}
