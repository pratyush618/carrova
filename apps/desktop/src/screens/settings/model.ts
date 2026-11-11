import type { BridgeError, MoveDefaults, OsKind } from '../../engine/types';

export type Appearance = 'system' | 'light' | 'dark';
export type MotionChoice = 'system' | 'reduce';

/** Where the recipes in use came from. */
export type RecipeSource =
  { readonly kind: 'dev' } | { readonly kind: 'installed' | 'shipped'; readonly version: number };

/** The last "Check now" for recipes, and the install of a list it offered. */
export type RecipeCheck =
  | { readonly status: 'idle' | 'checking' | 'up_to_date' | 'retry_later' }
  | {
      readonly status: 'available' | 'installing' | 'updated' | 'refused';
      readonly version: number;
    }
  | { readonly status: 'failed'; readonly error: BridgeError };

/** One stored error report (redacted by the engine). */
export interface ReportLine {
  readonly id: string;
  readonly when: Date;
  readonly headline: string;
}

/** Settings the bridge persists, plus recipes, updates and error reports. */
export interface SettingsModel {
  readonly now: Date;
  readonly appearance: Appearance;
  readonly motion: MotionChoice;
  readonly autoCheckRecipes: boolean;
  readonly autoCheckApp: boolean;
  readonly recipes: {
    readonly source: RecipeSource;
    readonly verified: number;
    readonly community: number;
    /** Recipes from an unsigned developer folder. */
    readonly unverified: number;
    readonly check: RecipeCheck;
  } | null;
  /** Keep local error reports (consent). */
  readonly errorReports: boolean;
  readonly reports: readonly ReportLine[];
  /** A newer Carrova the last check found, if any. */
  readonly update: { readonly version: string } | null;
  /** What every new move starts with (each move can still change it). */
  readonly moveDefaults: MoveDefaults;
  /** Send opens in Manual mode. */
  readonly startInManual: boolean;
  /** "Send with Carrova" in the file manager; `null` hides the row (this OS has no entries). */
  readonly fileManager: {
    readonly on: boolean;
    /** File managers left untouched and why (the engine's words). */
    readonly skipped: readonly string[];
    /** The last change could not be made. */
    readonly failed: boolean;
  } | null;
}

export interface SettingsIntents {
  readonly onAppearance: (value: Appearance) => void;
  readonly onMotion: (value: MotionChoice) => void;
  readonly onAutoCheckRecipes: (on: boolean) => void;
  readonly onAutoCheckApp: (on: boolean) => void;
  readonly onCheckRecipes: () => void;
  /** Installs the newer list the last check offered. */
  readonly onInstallRecipes: () => void;
  readonly onErrorReports: (on: boolean) => void;
  readonly onOpenReport: (id: string) => void;
  readonly onClearReports: () => void;
  readonly onUpdates: () => void;
  readonly onAbout: () => void;
  readonly onMoveDefaults: (patch: Partial<MoveDefaults>) => void;
  readonly onStartInManual: (on: boolean) => void;
  /** Write (`true`) or remove the file-manager entries. */
  readonly onFileManager: (on: boolean) => void;
}

/** The app update dialog: checking, nothing new, an offer, installing, or done. */
export type UpdateModel =
  | { readonly status: 'checking' | 'up_to_date' }
  | { readonly status: 'available'; readonly version: string; readonly notes: string | null }
  | { readonly status: 'installing' | 'installed'; readonly version: string }
  | { readonly status: 'failed'; readonly error: BridgeError };

export interface UpdateIntents {
  readonly onLater: () => void;
  readonly onInstall: () => void;
  readonly onRestart: () => void;
}

/** One error report, exactly as it would be sent, and the issue text once prepared. */
export interface CrashModel {
  readonly report: readonly string[];
  /** The prefilled GitHub issue, shown in full before it is opened. */
  readonly draft: { readonly title: string; readonly body: string } | null;
}

export interface CrashIntents {
  readonly onDontSend: () => void;
  /** Prepares the issue text, then (second press) opens it in the browser. */
  readonly onSend: () => void;
  readonly onDelete?: () => void;
}

export interface AboutModel {
  /** Unknown until the engine reports it. */
  readonly version: string | null;
  readonly engineVersion: string | null;
  /** This computer's name, when the system tells it. */
  readonly device: string | null;
  readonly os: OsKind | null;
  readonly licence: string;
}

export interface AboutIntents {
  readonly onSource?: () => void;
  readonly onThirdParty?: () => void;
  readonly onClose: () => void;
}
