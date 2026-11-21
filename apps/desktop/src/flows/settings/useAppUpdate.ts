import { useCallback, useMemo, useState } from 'react';
import { useUpdateStep } from '../../app/state/EngineState';
import type { CarrovaPort, UpdateStatus, UpdateStep } from '../../engine/types';
import type { UpdateModel } from '../../screens/settings/model';
import { current, type Remote } from '../remote';
import { useCommand } from '../useRemote';
import { updateModel } from './settingsViews';

export interface AppUpdateController {
  readonly check: Remote<UpdateStatus>;
  readonly model: UpdateModel;
  /** When a check last answered in this run (ms since epoch); `null` before. */
  readonly checkedAt: number | null;
  /** Checks the feed again; not while an install runs. */
  readonly runCheck: () => void;
  /** Installs exactly the version the check offered; never without this click. */
  readonly install: () => void;
  /** As `install`, and a phone may download it on mobile data. */
  readonly installOnMobileData: () => void;
  readonly cancel: () => void;
  /** Goes on now: installs while a move or call runs, or (a phone) downloads on mobile data. */
  readonly installAnyway: () => void;
  /** Holds (`true`) or carries on with a phone's download. */
  readonly pause: (paused: boolean) => void;
  /** Opens Android's "Install unknown apps" page for Carrova. */
  readonly openPermission: () => void;
  readonly restart: () => void;
}

export function useAppUpdate(port: CarrovaPort): AppUpdateController {
  const check = useCommand(useCallback(() => port.checkUpdate(), [port]));
  const install = useCommand(
    useCallback(
      (version: string, mobileData: boolean) => port.installUpdate(version, mobileData),
      [port],
    ),
  );
  const step = useUpdateStep();
  // The step a new install started with belongs to the one before it.
  const [stale, setStale] = useState<UpdateStep | null>(null);
  const [checkedAt, setCheckedAt] = useState<number | null>(null);
  const offer = current(check.state);
  const version = offer?.status === 'available' ? offer.version : null;
  const runCheck = check.run;
  const runInstall = install.run;
  const installing = install.state.status === 'loading';
  return useMemo(() => {
    const start = (mobileData: boolean) => {
      if (version === null) return;
      setStale(step);
      void runInstall(version, mobileData);
    };
    return {
      check: check.state,
      model: updateModel(check.state, install.state, version, step === stale ? null : step),
      checkedAt,
      runCheck: () => {
        if (installing) return;
        void runCheck().then((answer) => {
          if (answer !== null) setCheckedAt(Date.now());
        });
      },
      install: () => {
        start(false);
      },
      installOnMobileData: () => {
        start(true);
      },
      cancel: () => void port.cancelUpdate(),
      installAnyway: () => void port.installUpdateAnyway(),
      pause: (paused) => void port.pauseUpdate(paused),
      openPermission: () => void port.openInstallPermission(),
      restart: () => void port.restartApp(),
    };
  }, [
    check.state,
    install.state,
    version,
    step,
    stale,
    checkedAt,
    installing,
    runCheck,
    runInstall,
    port,
  ]);
}
