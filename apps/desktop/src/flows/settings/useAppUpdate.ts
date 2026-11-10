import { useCallback, useMemo } from 'react';
import type { CarrovaPort, UpdateStatus } from '../../engine/types';
import type { UpdateModel } from '../../screens/settings/model';
import { current, type Remote } from '../remote';
import { useCommand } from '../useRemote';
import { updateModel } from './settingsViews';

export interface AppUpdateController {
  readonly check: Remote<UpdateStatus>;
  readonly model: UpdateModel;
  readonly runCheck: () => void;
  /** Installs exactly the version the check offered; never without this click. */
  readonly install: () => void;
  readonly restart: () => void;
}

export function useAppUpdate(port: CarrovaPort): AppUpdateController {
  const check = useCommand(useCallback(() => port.checkUpdate(), [port]));
  const install = useCommand(useCallback((version: string) => port.installUpdate(version), [port]));
  const offer = current(check.state);
  const version = offer?.status === 'available' ? offer.version : null;
  const runCheck = check.run;
  const runInstall = install.run;
  return useMemo(
    () => ({
      check: check.state,
      model: updateModel(check.state, install.state, version),
      runCheck: () => void runCheck(),
      install: () => {
        if (version !== null) void runInstall(version);
      },
      restart: () => void port.restartApp(),
    }),
    [check.state, install.state, version, runCheck, runInstall, port],
  );
}
