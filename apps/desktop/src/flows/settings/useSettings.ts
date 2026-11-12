import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { BackgroundSettings } from '../../bindings/BackgroundSettings';
import type { FileManagerStatus } from '../../bindings/FileManagerStatus';
import type {
  About,
  CarrovaPort,
  RecipesStatus,
  ReportConsent,
  Settings,
  SettingsPatch,
  UpdateStatus,
} from '../../engine/types';
import { withPatch } from '../../ipc/settingsPatch';
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
  /** The last write or removal of the file-manager entries failed. */
  readonly fileManagerFailed: boolean;
  /** Save the changed fields only, on top of the settings as saved then. */
  readonly save: (patch: SettingsPatch) => void;
  /** Change the background settings (and start at sign-in with them); a refused change goes back to the stored one. */
  readonly setBackground: (patch: Partial<BackgroundSettings>) => void;
  /** Write (`true`) or remove the file-manager entries; once done, the one-time question counts as answered. */
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
    (patch: SettingsPatch) => {
      if (settings === null) return;
      setEdited(withPatch(settings, patch));
      void port.saveSettings(patch);
    },
    [port, settings],
  );

  const reload = loaded.run;
  const setBackground = useCallback(
    (patch: Partial<BackgroundSettings>) => {
      if (settings === null) return;
      setEdited({ ...settings, background: { ...settings.background, ...patch } });
      void port.setBackground(patch).catch(() => {
        setEdited(null);
        void reload();
      });
    },
    [port, settings, reload],
  );

  // The write answers later: save on top of the settings as they are then.
  const saveLatest = useRef(save);
  useEffect(() => {
    saveLatest.current = save;
  });
  const runSetEntries = setEntries.run;
  const setFileManager = useCallback(
    (on: boolean) => {
      // A failed write leaves the question open, so Home can offer it again.
      void runSetEntries(on).then((status) => {
        if (status !== null) saveLatest.current({ file_manager_asked: true });
      });
    },
    [runSetEntries],
  );
  const fileManagerFailed = setEntries.state.status === 'failed';

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
              fileManagerFailed,
              consent: consentValue,
              reports: current(reports.state) ?? [],
              recipes: current(recipes.state),
              check: check.state,
              install: install.state,
              update,
              about: current(about.state),
            },
            new Date(),
          ),
    [
      settings,
      fileManager,
      fileManagerFailed,
      consentValue,
      reports.state,
      recipes.state,
      check.state,
      install.state,
      update,
      about.state,
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
      fileManagerFailed,
      save,
      setBackground,
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
      fileManagerFailed,
      save,
      setBackground,
      setFileManager,
      setConsent,
      checkRecipes,
      installRecipes,
      clearReports,
      reloadAll,
    ],
  );
}
