/**
 * The engine port: every operation the UI can ask of the engine, and the one
 * event stream it receives. Fixed by the Phase 4 contract (main thread).
 *
 * Screens and app state depend only on this interface. The Tauri adapter
 * (`./tauri.ts`) implements it over `invoke` + the `carrova://event` channel;
 * `./fake/` implements it in memory for tests, Playwright and the gallery.
 *
 * All payload types are generated from Rust (`../bindings`). Failed calls
 * reject with a `BridgeError`. Long-running calls (`exportArchive`, `apply`,
 * `runSend`, `connect`) resolve when the engine is done; progress arrives
 * meanwhile as `BridgeEvent`s and `cancel(session)` stops them between steps
 * (never mid-apply).
 */
import type { About } from '../bindings/About';
import type { ArchiveSummary } from '../bindings/ArchiveSummary';
import type { BackgroundSettings } from '../bindings/BackgroundSettings';
import type { BridgeEvent } from '../bindings/BridgeEvent';
import type { CatalogEntry } from '../bindings/CatalogEntry';
import type { Checklist } from '../bindings/Checklist';
import type { ErrorReport } from '../bindings/ErrorReport';
import type { FileManagerStatus } from '../bindings/FileManagerStatus';
import type { IssueDraft } from '../bindings/IssueDraft';
import type { KdfStrength } from '../bindings/KdfStrength';
import type { NearbyPeer } from '../bindings/NearbyPeer';
import type { Picked } from '../bindings/Picked';
import type { Notice } from '../bindings/Notice';
import type { NoticeId } from '../bindings/NoticeId';
import type { PlanEdit } from '../bindings/PlanEdit';
import type { PlanOptions } from '../bindings/PlanOptions';
import type { PassphraseStrength } from '../bindings/PassphraseStrength';
import type { PlanReview } from '../bindings/PlanReview';
import type { Preset } from '../bindings/Preset';
import type { ReportConsent } from '../bindings/ReportConsent';
import type { ReportId } from '../bindings/ReportId';
import type { ReportSummary } from '../bindings/ReportSummary';
import type { RecipesCheck } from '../bindings/RecipesCheck';
import type { RecipesStatus } from '../bindings/RecipesStatus';
import type { RecipesUpdate } from '../bindings/RecipesUpdate';
import type { RollbackPreview } from '../bindings/RollbackPreview';
import type { RollbackReport } from '../bindings/RollbackReport';
import type { ScanResult } from '../bindings/ScanResult';
import type { Selection } from '../bindings/Selection';
import type { SelectionEdit } from '../bindings/SelectionEdit';
import type { SendReport } from '../bindings/SendReport';
import type { SessionId } from '../bindings/SessionId';
import type { SessionOpened } from '../bindings/SessionOpened';
import type { SessionRecord } from '../bindings/SessionRecord';
import type { Settings } from '../bindings/Settings';
import type { SettingsPatch } from './settingsPatch';
import type { UpdateStatus } from '../bindings/UpdateStatus';

/** Stops a subscription. */
export type Unsubscribe = () => void;

export interface CarrovaPort {
  // --- Events -------------------------------------------------------------
  /** Receive every bridge event (engine events, pairing prompts, task ends). */
  subscribe(listener: (event: BridgeEvent) => void): Promise<Unsubscribe>;

  // --- Sending side -------------------------------------------------------
  /** Scan this machine and open an export session with a suggested selection. */
  scan(): Promise<ScanResult>;
  /** Open an export session in Manual mode: no app scan, an empty selection. */
  startManual(): Promise<ScanResult>;
  /** Run the app scan of a Manual session (first switch to Auto); keeps its picks. */
  scanApps(session: SessionId): Promise<ScanResult>;
  /**
   * Add the absolute `paths` the user picked to `selection` by the engine's
   * rules; resolves with the result and one outcome (added, absorbed,
   * duplicate or rejected with a reason) per path.
   */
  pick(session: SessionId, selection: Selection, paths: string[]): Promise<Picked>;
  /** Paths sent from outside (file manager, a second launch) since the last call. */
  takeSendRequest(): Promise<string[]>;
  /** The selection a preset suggests (the session keeps its own until `select`). */
  suggest(session: SessionId, preset: Preset): Promise<ScanResult>;
  /**
   * Apply the user's `edits` to `selection` by the engine's rules and make the
   * result the session's selection; resolves with it and its view (issues and
   * all). The engine refuses export and send while the selection has issues.
   * Rejects with `invalid_selection` (nothing changed) if an edit names an
   * unknown item.
   */
  select(session: SessionId, selection: Selection, edits: SelectionEdit[]): Promise<ScanResult>;
  /** How strong `passphrase` is, by the rule `exportArchive` enforces. */
  passphraseStrength(passphrase: string): Promise<PassphraseStrength>;
  /**
   * Write the encrypted archive file, its passphrase protected at `strength`
   * (`standard` is the suggestion; stronger costs time and memory to open).
   */
  exportArchive(
    session: SessionId,
    path: string,
    passphrase: string,
    strength: KdfStrength,
  ): Promise<ArchiveSummary>;
  /**
   * Start a LAN send: captures, advertises and emits `pairing_code`, then runs
   * until the receiver finishes. `confirm_new_code` prompts are answered with
   * `answerPairing`.
   */
  runSend(session: SessionId): Promise<SendReport>;
  /** Answer a `confirm_new_code` prompt. */
  answerPairing(session: SessionId, offerNewCode: boolean): Promise<void>;

  // --- Receiving side -----------------------------------------------------
  /** Open an archive file as an import session. */
  openArchive(path: string, passphrase: string): Promise<SessionOpened>;
  /** Pair with a sender by its code (or `code@ip:port` ticket). */
  connect(code: string): Promise<SessionOpened>;
  /** Stop a `connect` that is still pairing (it has no session yet). */
  cancelConnect(): Promise<void>;
  /** Senders nearby offering a code (device names only; the user still types the code). */
  browseNearby(): Promise<NearbyPeer[]>;
  /** (Re)build the plan with the user's options; nothing is written. */
  plan(session: SessionId, options: PlanOptions): Promise<PlanReview>;
  /**
   * Re-plan with the user's `edits` applied by the engine to the current plan's
   * options (`review.plan.options` holds the result). Rejects with
   * `invalid_selection` (nothing changed) before the first `plan` or if an edit
   * names an unknown item or plugin.
   */
  editPlan(session: SessionId, edits: PlanEdit[]): Promise<PlanReview>;
  /**
   * The options the session last planned with (`null` before its first plan).
   * A resumed import plans from these, so its earlier choices and approval
   * carry over; Settings defaults only pre-fill new moves.
   */
  planOptions(session: SessionId): Promise<PlanOptions | null>;
  /** Approve the current plan (rejects with `blocked` if it has blockers). */
  approve(session: SessionId): Promise<void>;
  /** Apply the approved plan: staged, journalled, reversible. */
  apply(session: SessionId): Promise<Checklist>;
  /** End a review without applying (dry run). */
  endReview(session: SessionId): Promise<void>;
  /** Hold (`true`) or continue (`false`) a running LAN transfer between batches. */
  setTransferPaused(session: SessionId, paused: boolean): Promise<void>;
  /**
   * Continue a LAN transfer paused for low disk space, ignoring the free-space
   * check (the user's override, after the warning). A really full disk still stops it.
   */
  ignoreFreeSpace(session: SessionId): Promise<void>;
  /** Reconnect a lost LAN transfer now instead of after the back-off ("Try now"). */
  retryConnection(session: SessionId): Promise<void>;

  // --- Any session --------------------------------------------------------
  /** Stop a running task between steps; apply always completes or rolls back. */
  cancel(session: SessionId): Promise<void>;
  /**
   * Resume an interrupted session (LAN or archive). An archive import needs
   * its passphrase again: without one it rejects with `wrong_passphrase`.
   */
  resume(session: SessionId, passphrase?: string): Promise<SessionOpened>;
  /** History, newest first. */
  sessions(): Promise<SessionRecord[]>;
  /** Undo an applied session (newest when `session` is null). */
  rollback(session: SessionId | null): Promise<RollbackReport>;
  /** What `rollback` would do now (counts and blockers); changes nothing. */
  rollbackPreview(session: SessionId | null): Promise<RollbackPreview>;
  /** Forget a session and its staged data. */
  dismiss(session: SessionId): Promise<void>;

  // --- Notifications ------------------------------------------------------
  /** The bell's notices (kept 30 days), newest first. */
  notices(): Promise<Notice[]>;
  /** Mark notices read: the given ones, or `'all'`. */
  markNoticesRead(ids: NoticeId[] | 'all'): Promise<void>;

  // --- Recipes ------------------------------------------------------------
  /** Apps Carrova knows, with detection on this machine. */
  catalog(): Promise<CatalogEntry[]>;
  recipesStatus(): Promise<RecipesStatus>;
  /** Whether a newer recipe bundle is offered; installs nothing. */
  recipesCheck(): Promise<RecipesCheck>;
  /** Check for, verify and install a newer recipe bundle. */
  recipesUpdate(): Promise<RecipesUpdate>;
  /** Remove the installed recipe bundle; the recipes shipped with the app are used again. */
  recipesReset(): Promise<RecipesStatus>;

  // --- This app -----------------------------------------------------------
  /** Versions, this device's name (if known) and OS, for the About sheet. */
  about(): Promise<About>;

  // --- Settings -----------------------------------------------------------
  settings(): Promise<Settings>;
  /** Save the changed fields (see {@link SettingsPatch}). */
  saveSettings(patch: SettingsPatch): Promise<void>;
  /** Whether the "Send with Carrova" file-manager entries are written. */
  fileManagerStatus(): Promise<FileManagerStatus>;
  /** Write (`true`) or remove (`false`) the file-manager entries. */
  setFileManagerEntries(on: boolean): Promise<FileManagerStatus>;
  /** Save the background settings and start at sign-in only while Keep running and Start at login are on. */
  setBackground(background: BackgroundSettings): Promise<void>;

  // --- App updates (never installed without the user's click) -------------
  checkUpdate(): Promise<UpdateStatus>;
  /** Install exactly the version that `checkUpdate` offered. */
  installUpdate(version: string): Promise<void>;
  restartApp(): Promise<void>;

  // --- Error reports (opt-in; nothing leaves the machine by itself) -------
  errorReportConsent(): Promise<ReportConsent>;
  setErrorReportConsent(consent: ReportConsent): Promise<void>;
  errorReports(): Promise<ReportSummary[]>;
  errorReport(id: ReportId): Promise<ErrorReport>;
  deleteErrorReport(id: ReportId): Promise<void>;
  clearErrorReports(): Promise<void>;
  /** Prefilled issue text + URL; the UI shows it, then opens the URL on request. */
  errorReportIssue(id: ReportId): Promise<IssueDraft>;
}
