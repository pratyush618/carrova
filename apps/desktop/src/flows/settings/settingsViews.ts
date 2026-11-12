import type { FileManagerStatus } from '../../bindings/FileManagerStatus';
import type {
  About,
  RecipesStatus,
  RecipesUpdate,
  ReportConsent,
  ReportSummary,
  Settings,
  UpdateStatus,
} from '../../engine/types';
import type { RecipesCheck } from '../../bindings/RecipesCheck';
import type {
  AboutModel,
  PauseChoice,
  RecipeCheck,
  RecipeSource,
  SettingsModel,
  UpdateModel,
} from '../../screens/settings/model';
import type { DevicesController } from '../nearby/useDevices';
import { current, type Remote } from '../remote';

/** Where the recipes came from, as Settings words it. */
export function recipeSource(status: RecipesStatus): RecipeSource {
  return status.origin.kind === 'dev_dir'
    ? { kind: 'dev' }
    : { kind: status.origin.kind, version: status.origin.version };
}

/**
 * "Check now" and, once a newer list is offered, "Install update": the
 * install's outcome wins once it has started. Feed unreachable = "try again later".
 */
export function recipeCheck(
  check: Remote<RecipesCheck>,
  install: Remote<RecipesUpdate>,
): RecipeCheck {
  const offered = current(check);
  switch (install.status) {
    case 'loading':
      return offered?.kind === 'available'
        ? { status: 'installing', version: offered.version }
        : { status: 'checking' };
    case 'failed':
      return { status: 'failed', error: install.error };
    case 'ready':
      return install.value.kind === 'updated'
        ? { status: 'updated', version: install.value.version }
        : { status: install.value.kind };
    case 'idle':
      break;
  }
  switch (check.status) {
    case 'idle':
      return { status: 'idle' };
    case 'loading':
      return { status: 'checking' };
    case 'failed':
      return { status: 'failed', error: check.error };
    case 'ready': {
      const v = check.value;
      return v.kind === 'available' || v.kind === 'refused'
        ? { status: v.kind, version: v.version }
        : { status: v.kind };
    }
  }
}

export interface SettingsSources {
  readonly settings: Settings;
  /** The file-manager entries; `null` until the engine answered. */
  readonly fileManager: FileManagerStatus | null;
  /** The last change to them failed. */
  readonly fileManagerFailed: boolean;
  readonly consent: ReportConsent;
  readonly reports: readonly ReportSummary[];
  readonly recipes: RecipesStatus | null;
  readonly check: Remote<RecipesCheck>;
  readonly install: Remote<RecipesUpdate>;
  readonly update: Remote<UpdateStatus>;
  /** This app and computer; `null` until the engine answered. */
  readonly about: About | null;
  /** Who can find this computer and how many devices it remembers. */
  readonly devices: Pick<DevicesController, 'visibility' | 'known'>;
}

const HOUR = 3600;

/** The Unix second pop-ups stay held back until, for a pause choice (`null` = not paused). */
export function pausedUntil(choice: PauseChoice, now: Date): number | null {
  const seconds = Math.floor(now.getTime() / 1000);
  switch (choice) {
    case 'off':
      return null;
    case 'hour':
      return seconds + HOUR;
    case 'tomorrow':
      return Math.floor(
        new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1).getTime() / 1000,
      );
  }
}

/** The Settings page from everything the engine says. */
export function settingsModel(s: SettingsSources, now: Date): SettingsModel {
  const offer = current(s.update);
  return {
    now,
    background: s.settings.background,
    minimisesOnClose: s.settings.background.keep_running && s.about?.status_tray === false,
    alerts: s.settings.alerts,
    pausedUntil:
      s.settings.alerts.paused_until !== null &&
      s.settings.alerts.paused_until * 1000 > now.getTime()
        ? new Date(s.settings.alerts.paused_until * 1000)
        : null,
    appearance: s.settings.theme ?? 'system',
    motion: s.settings.reduce_motion === true ? 'reduce' : 'system',
    autoCheckRecipes: s.settings.auto_check_recipes,
    autoCheckApp: s.settings.auto_check_app,
    recipes:
      s.recipes === null
        ? null
        : {
            source: recipeSource(s.recipes),
            verified: s.recipes.verified,
            community: s.recipes.community,
            unverified: s.recipes.unverified,
            check: recipeCheck(s.check, s.install),
          },
    errorReports: s.consent === 'allowed',
    reports: s.reports.map((r) => ({
      id: r.id,
      when: new Date(r.when * 1000),
      headline: r.headline,
    })),
    update: offer?.status === 'available' ? { version: offer.version } : null,
    moveDefaults: s.settings.move_defaults,
    startInManual: s.settings.start_in_manual,
    fileManager:
      s.fileManager?.supported === true
        ? {
            on: s.fileManager.installed,
            skipped: s.fileManager.skipped,
            failed: s.fileManagerFailed,
          }
        : null,
    nearby: s.settings.nearby,
    visibility: s.devices.visibility?.choice ?? null,
    knownDevices: s.devices.known.length,
  };
}

/**
 * Whether Home asks (once) to add "Send with Carrova" to the file manager:
 * this OS has entries, they aren't written, and the user never answered.
 */
export const offerFileManager = (
  settings: Settings | null,
  status: FileManagerStatus | null,
): boolean =>
  settings !== null &&
  !settings.file_manager_asked &&
  status !== null &&
  status.supported &&
  !status.installed;

/** The About sheet: this app, its engine and this computer, as the engine reports them. */
export function aboutModel(about: About | null): AboutModel {
  return {
    version: about?.app_version ?? null,
    engineVersion: about?.engine_version ?? null,
    device: about?.device_name ?? null,
    os: about?.os ?? null,
    licence: 'Apache-2.0',
  };
}

/** The update dialog from the check and install calls. */
export function updateModel(
  check: Remote<UpdateStatus>,
  install: Remote<void>,
  version: string | null,
): UpdateModel {
  if (version !== null) {
    switch (install.status) {
      case 'loading':
        return { status: 'installing', version };
      case 'ready':
        return { status: 'installed', version };
      case 'failed':
        return { status: 'failed', error: install.error };
      case 'idle':
        break;
    }
  }
  switch (check.status) {
    case 'idle':
    case 'loading':
      return { status: 'checking' };
    case 'failed':
      return { status: 'failed', error: check.error };
    case 'ready': {
      const v = check.value;
      if (v.status === 'available')
        return { status: 'available', version: v.version, notes: v.notes };
      if (v.status === 'installed') return { status: 'installed', version: v.version };
      return { status: 'up_to_date' };
    }
  }
}
