import type {
  About,
  RecipesStatus,
  RecipesUpdate,
  ReportConsent,
  ReportSummary,
  Settings,
  UpdateStatus,
} from '../../engine/types';
import type {
  AboutModel,
  RecipeCheck,
  RecipeSource,
  SettingsModel,
  UpdateModel,
} from '../../screens/settings/model';
import { current, type Remote } from '../remote';

/** Where the recipes came from, as Settings words it. */
export function recipeSource(status: RecipesStatus): RecipeSource {
  return status.origin.kind === 'dev_dir'
    ? { kind: 'dev' }
    : { kind: status.origin.kind, version: status.origin.version };
}

/** The outcome of "Check now", including "try again later" when the feed is unreachable. */
export function recipeCheck(check: Remote<RecipesUpdate>): RecipeCheck {
  switch (check.status) {
    case 'idle':
      return { status: 'idle' };
    case 'loading':
      return { status: 'checking' };
    case 'failed':
      return { status: 'failed', error: check.error };
    case 'ready':
      return check.value.kind === 'updated'
        ? { status: 'updated', version: check.value.version }
        : { status: check.value.kind };
  }
}

export interface SettingsSources {
  readonly settings: Settings;
  readonly consent: ReportConsent;
  readonly reports: readonly ReportSummary[];
  readonly recipes: RecipesStatus | null;
  readonly check: Remote<RecipesUpdate>;
  readonly update: Remote<UpdateStatus>;
}

/** The Settings page from everything the engine says. */
export function settingsModel(s: SettingsSources, now: Date): SettingsModel {
  const offer = current(s.update);
  return {
    now,
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
            check: recipeCheck(s.check),
          },
    errorReports: s.consent === 'allowed',
    reports: s.reports.map((r) => ({
      id: r.id,
      when: new Date(r.when * 1000),
      headline: r.headline,
    })),
    update: offer?.status === 'available' ? { version: offer.version } : null,
    moveDefaults: s.settings.move_defaults,
  };
}

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
