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
  /** Checks the feed again; not while an install runs. */
  readonly runCheck: () => void;
  /** Installs exactly the version the check offered; never without this click. */
  readonly install: () => void;
  readonly cancel: () => void;
  /** Installs the waiting update while a move or call runs. */
  readonly installAnyway: () => void;
  readonly restart: () => void;
}

export function useAppUpdate(port: CarrovaPort): AppUpdateController {
  const check = useCommand(useCallback(() => port.checkUpdate(), [port]));
  const install = useCommand(useCallback((version: string) => port.installUpdate(version), [port]));
  const step = useUpdateStep();
  // The step a new install started with belongs to the one before it.
  const [stale, setStale] = useState<UpdateStep | null>(null);
  const offer = current(check.state);
  const version = offer?.status === 'available' ? offer.version : null;
  const runCheck = check.run;
  const runInstall = install.run;
  const installing = install.state.status === 'loading';
  return useMemo(
    () => ({
      check: check.state,
      model: updateModel(check.state, install.state, version, step === stale ? null : step),
      runCheck: () => {
        if (!installing) void runCheck();
      },
      install: () => {
        if (version === null) return;
        setStale(step);
        void runInstall(version);
      },
      cancel: () => void port.cancelUpdate(),
      installAnyway: () => void port.installUpdateAnyway(),
      restart: () => void port.restartApp(),
    }),
    [check.state, install.state, version, step, stale, installing, runCheck, runInstall, port],
  );
}
