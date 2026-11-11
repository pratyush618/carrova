import { useCallback, useEffect, useMemo, useState } from 'react';
import type { FileManagerStatus } from '../../bindings/FileManagerStatus';
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
import { offerFileManager, settingsModel } from './settingsViews';
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
  /** Home asks once about "Send with Carrova" in the file manager. */
  readonly offerFileManager: boolean;
  readonly save: (patch: Partial<Settings>) => void;
  /** Write (`true`) or remove the file-manager entries; the one-time question counts as answered. */
  readonly setFileManager: (on: boolean) => void;
  readonly setConsent: (allowed: boolean) => void;
  readonly checkRecipes: () => void;
  readonly installRecipes: () => void;
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
  const entries = useRemote(useCallback(() => port.fileManagerStatus(), [port]));
  const setEntries = useCommand(
    useCallback((on: boolean) => port.setFileManagerEntries(on), [port]),
  );
  const fileManager: FileManagerStatus | null = current(setEntries.state) ?? current(entries.state);
  const check = useCommand(useCallback(() => port.recipesCheck(), [port]));
  const install = useCommand(useCallback(() => port.recipesUpdate(), [port]));
  const [edited, setEdited] = useState<Settings | null>(null);
  const settings = edited ?? current(loaded.state);

  const reloadRecipes = recipes.run;
  useEffect(() => {
    if (install.state.status === 'ready' && install.state.value.kind === 'updated')
      void reloadRecipes();
  }, [install.state, reloadRecipes]);

  const save = useCallback(
    (patch: Partial<Settings>) => {
      if (settings === null) return;
      const next = { ...settings, ...patch };
      setEdited(next);
      void port.saveSettings(next);
    },
    [port, settings],
  );

  const runSetEntries = setEntries.run;
  const setFileManager = useCallback(
    (on: boolean) => {
      save({ file_manager_asked: true });
      void runSetEntries(on);
    },
    [save, runSetEntries],
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
              fileManager,
              fileManagerFailed: setEntries.state.status === 'failed',
              consent: consentValue,
              reports: current(reports.state) ?? [],
              recipes: current(recipes.state),
              check: check.state,
              install: install.state,
              update,
            },
            new Date(),
          ),
    [
      settings,
      fileManager,
      setEntries.state.status,
      consentValue,
      reports.state,
      recipes.state,
      check.state,
      install.state,
      update,
    ],
  );

  const runCheck = check.run;
  const { run: runInstall, reset: resetInstall } = install;
  const checkRecipes = useCallback(() => {
    resetInstall();
    void runCheck();
  }, [runCheck, resetInstall]);
  const installRecipes = useCallback(() => void runInstall(), [runInstall]);
  const reloadAll = useCallback(() => void reloadReports(), [reloadReports]);
  return useMemo(
    () => ({
      model,
      settings,
      consent: consentValue,
      recipes: current(recipes.state),
      about: current(about.state),
      offerFileManager: offerFileManager(settings, fileManager),
      save,
      setFileManager,
      setConsent,
      checkRecipes,
      installRecipes,
      clearReports,
      reloadReports: reloadAll,
    }),
    [
      model,
      settings,
      consentValue,
      recipes.state,
      about.state,
      fileManager,
      save,
      setFileManager,
      setConsent,
      checkRecipes,
      installRecipes,
      clearReports,
      reloadAll,
    ],
  );
}
