import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  About,
  CarrovaPort,
  RecipesStatus,
  ReportConsent,
  Settings,
  UpdateStatus,
} from '../../engine/types';
import { current, type Remote } from '../remote';
import { useCommand, useRemote } from '../useRemote';
import { settingsModel } from './settingsViews';
import type { SettingsModel } from '../../screens/settings/model';

export interface SettingsController {
  /** `null` until settings and consent have loaded. */
  readonly model: SettingsModel | null;
  readonly settings: Settings | null;
  /** Error-report consent (`unset` = never asked); `null` until loaded. */
  readonly consent: ReportConsent | null;
  /** The recipes in use (reloaded after an update); `null` until loaded. */
  readonly recipes: RecipesStatus | null;
  /** This app and computer (About sheet); `null` until the engine answered. */
  readonly about: About | null;
  readonly save: (patch: Partial<Settings>) => void;
  readonly setConsent: (allowed: boolean) => void;
  readonly checkRecipes: () => void;
  readonly clearReports: () => void;
  readonly reloadReports: () => void;
}

/** Settings, recipes status and error-report consent; every change is saved through the port. */
export function useSettings(port: CarrovaPort, update: Remote<UpdateStatus>): SettingsController {
  const loaded = useRemote(useCallback(() => port.settings(), [port]));
  const consent = useRemote(useCallback(() => port.errorReportConsent(), [port]));
  const reports = useRemote(useCallback(() => port.errorReports(), [port]));
  const recipes = useRemote(useCallback(() => port.recipesStatus(), [port]));
  const about = useRemote(useCallback(() => port.about(), [port]));
  const check = useCommand(useCallback(() => port.recipesUpdate(), [port]));
  const [edited, setEdited] = useState<Settings | null>(null);
  const settings = edited ?? current(loaded.state);

  const reloadRecipes = recipes.run;
  useEffect(() => {
    if (check.state.status === 'ready' && check.state.value.kind === 'updated')
      void reloadRecipes();
  }, [check.state, reloadRecipes]);

  const save = useCallback(
    (patch: Partial<Settings>) => {
      if (settings === null) return;
      const next = { ...settings, ...patch };
      setEdited(next);
      void port.saveSettings(next);
    },
    [port, settings],
  );

  const reloadConsent = consent.run;
  const setConsent = useCallback(
    (allowed: boolean) => {
      void port.setErrorReportConsent(allowed ? 'allowed' : 'denied').then(() => reloadConsent());
    },
    [port, reloadConsent],
  );

  const reloadReports = reports.run;
  const clearReports = useCallback(() => {
    void port.clearErrorReports().then(() => reloadReports());
  }, [port, reloadReports]);

  const consentValue = current(consent.state);
  const model = useMemo(
    () =>
      settings === null || consentValue === null
        ? null
        : settingsModel(
            {
              settings,
              consent: consentValue,
              reports: current(reports.state) ?? [],
              recipes: current(recipes.state),
              check: check.state,
              update,
            },
            new Date(),
          ),
    [settings, consentValue, reports.state, recipes.state, check.state, update],
  );

  const runCheck = check.run;
  const checkRecipes = useCallback(() => void runCheck(), [runCheck]);
  const reloadAll = useCallback(() => void reloadReports(), [reloadReports]);
  return useMemo(
    () => ({
      model,
      settings,
      consent: consentValue,
      recipes: current(recipes.state),
      about: current(about.state),
      save,
      setConsent,
      checkRecipes,
      clearReports,
      reloadReports: reloadAll,
    }),
    [
      model,
      settings,
      consentValue,
      recipes.state,
      about.state,
      save,
      setConsent,
      checkRecipes,
      clearReports,
      reloadAll,
    ],
  );
}
