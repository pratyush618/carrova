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
import type { AskId } from '../bindings/AskId';
import type { ArchiveSummary } from '../bindings/ArchiveSummary';
import type { BackgroundSettings } from '../bindings/BackgroundSettings';
import type { BridgeEvent } from '../bindings/BridgeEvent';
import type { CallAction } from '../bindings/CallAction';
import type { CallLogView } from '../bindings/CallLogView';
import type { CallOutcome } from '../bindings/CallOutcome';
import type { CallSetupStep } from '../bindings/CallSetupStep';
import type { CallView } from '../bindings/CallView';
import type { CallWindowView } from '../bindings/CallWindowView';
import type { Contact } from '../bindings/Contact';
import type { Cursor } from '../bindings/Cursor';
import type { LocalClock } from '../bindings/LocalClock';
import type { NoteAction } from '../bindings/NoteAction';
import type { NoteOutcome } from '../bindings/NoteOutcome';
import type { NotesView } from '../bindings/NotesView';
import type { NotifyAppRow } from '../bindings/NotifyAppRow';
import type { NotifyAppsView } from '../bindings/NotifyAppsView';
import type { PermStep } from '../bindings/PermStep';
import type { PhonePerm } from '../bindings/PhonePerm';
import type { PhoneSettings } from '../bindings/PhoneSettings';
import type { SmsMessage } from '../bindings/SmsMessage';
import type { SimChoice } from '../bindings/SimChoice';
import type { SmsSendOutcome } from '../bindings/SmsSendOutcome';
import type { SyncWindow } from '../bindings/SyncWindow';
import type { ThreadView } from '../bindings/ThreadView';
import type { ThreadsView } from '../bindings/ThreadsView';
import type { CatalogEntry } from '../bindings/CatalogEntry';
import type { ClipHistoryView } from '../bindings/ClipHistoryView';
import type { ClipSettings } from '../bindings/ClipSettings';
import type { Checklist } from '../bindings/Checklist';
import type { ErrorReport } from '../bindings/ErrorReport';
import type { Feature } from '../bindings/Feature';
import type { FeatureState } from '../bindings/FeatureState';
import type { FileManagerStatus } from '../bindings/FileManagerStatus';
import type { ForgetDevices } from '../bindings/ForgetDevices';
import type { IssueDraft } from '../bindings/IssueDraft';
import type { KdfStrength } from '../bindings/KdfStrength';
import type { KnownDeviceView } from '../bindings/KnownDeviceView';
import type { LinkedDeviceView } from '../bindings/LinkedDeviceView';
import type { NearbyDeviceView } from '../bindings/NearbyDeviceView';
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
import type { RequestAnswer } from '../bindings/RequestAnswer';
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
import type { VisibilityChoice } from '../bindings/VisibilityChoice';
import type { VisibilityView } from '../bindings/VisibilityView';

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

  // --- Code-less moves between trusted computers (`about().known_send`) ---
  /**
   * Ask trusted computer `to` (a device id) to take this session's move
   * without a code. Once someone there opens Review it connects
   * (`peer_connected`) and the move runs as `runSend`'s does. Rejects with
   * `request_declined`, `request_expired` (2 minutes passed),
   * `request_refused` (it needs a code now), `network` (it can't be reached)
   * or `cancelled` (`cancel(session)`).
   */
  requestSend(session: SessionId, to: string): Promise<SendReport>;
  /**
   * Answer a `move_request_arrived`: `review` opens its import session (plan
   * it as after `connect`), `decline` ends it and resolves with `null`.
   * Rejects with `session` once the request has ended.
   */
  answerRequest(id: number, answer: RequestAnswer): Promise<SessionOpened | null>;
  /** Let known computer `device` send move requests (off: it needs a code, and its requests are dropped unseen). */
  setSendRequests(device: string, on: boolean): Promise<void>;

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

  // --- Nearby and known devices -----------------------------------------
  /** Devices nearby now (presences and code offers; a known one listed once), by answer time. */
  nearby(): Promise<NearbyDeviceView[]>;
  /** Devices this computer remembers. */
  knownDevices(): Promise<KnownDeviceView[]>;
  /** Rename a known `device` (id or fingerprint) on this computer only. */
  renameDevice(device: string, name: string): Promise<void>;
  /** Forget one known device, or `'all'`; it needs a code next time. */
  forgetDevice(which: ForgetDevices): Promise<void>;
  /** Give this computer a new identity: every device forgets it, it forgets them all. */
  resetIdentity(): Promise<void>;
  /** Who can find this computer nearby. */
  visibility(): Promise<VisibilityView>;
  /** Change who can find this computer; resolves with the visibility now in force. */
  setVisibility(choice: VisibilityChoice): Promise<VisibilityView>;
  /**
   * Wake a sleeping known `device` (Wake-on-LAN). Rejects with `known_device`
   * when it has no saved network address.
   */
  wakeDevice(device: string): Promise<void>;
  /**
   * Whether the tray asked for the Nearby page since the last call (asked at
   * start, then on each `show_nearby`; a window built for it was not listening).
   */
  takeShowNearby(): Promise<boolean>;

  // --- Linked devices (their events arrive on the `link` session) --------
  /** Devices linked to this computer, with each feature's settings on both sides. */
  linkedDevices(): Promise<LinkedDeviceView[]>;
  /**
   * Invite a phone: emits `pair_invite` (QR code and typed code), then
   * resolves once a device has linked. Stop it with `cancelConnect`.
   */
  pairInvite(): Promise<void>;
  /**
   * Answer the key check of `pair_invite` (`pair_peer_found`): link the
   * device found, or not (then nothing is stored on either side).
   */
  answerPairPeer(accept: boolean): Promise<void>;
  /** Link with another computer by its code (or `code@ip:port` ticket). Stop it with `cancelConnect`. */
  linkComputer(ticket: string): Promise<void>;
  /** Link with the computer whose `carrova://pair?…` link was scanned or pasted. Stop it with `cancelConnect`. */
  linkInvite(uri: string): Promise<void>;
  /** This device's own key fingerprint, shown while it links. */
  ownFingerprint(): Promise<string>;
  /** Scan a computer's pairing QR code with the phone's camera: its `carrova://pair?…` link, `null` if cancelled. */
  scanQr(): Promise<string | null>;
  /**
   * Links the phone app was opened with (share sheet, tile, notification)
   * since the last call, oldest first: `carrova://send-clipboard`, `file:`
   * links to shared copies, `http(s)` and `data:text/plain` text.
   */
  takeOpened(): Promise<string[]>;
  /** Be told when opened links are waiting for `takeOpened`. */
  onOpened(listener: () => void): Promise<Unsubscribe>;
  /** Whether this device has a LAN address now (`false`: the phone is not on Wi-Fi). */
  lanAvailable(): Promise<boolean>;
  /** Change one feature on this computer's side of the link with `id`. */
  setFeature(id: string, feature: Feature, state: FeatureState): Promise<void>;
  /** Answer a device's offer (`ask_received`). */
  answerAsk(askId: AskId, accept: boolean): Promise<void>;
  /** Send this computer's clipboard to `id` now. */
  sendClipboard(id: string): Promise<void>;
  /** Offer files to `id`, one at a time (it asks on arrival). */
  sendFiles(id: string, paths: string[]): Promise<void>;
  /** Offer an `http(s)` link to open on `id` (it asks on arrival). */
  handoffUrl(id: string, url: string): Promise<void>;
  /** Make the phone `id` ring. */
  ring(id: string): Promise<void>;
  /** Send `text` to `id` as a clip (this device's clipboard stays as it is). */
  sendText(id: string, text: string): Promise<void>;
  /** Stop the ring this computer started on `id`. */
  stopRing(id: string): Promise<void>;
  /** Clips received from linked devices, newest first. */
  clipHistory(): Promise<ClipHistoryView>;
  clearClipHistory(): Promise<void>;
  /** How this computer's clipboard is shared (Settings → Clipboard). */
  clipSettings(): Promise<ClipSettings>;
  /** Save and apply them at once (turning the history file off deletes it). */
  setClipSettings(settings: ClipSettings): Promise<void>;
  /** Send file `id` again, from the start: one whose device left (`file_failed` with `retry`). */
  retryFile(id: number): Promise<void>;

  // --- A linked phone on this computer (memory only; events on `link`) ---
  /** The notifications phone `id` shows: only a count while this computer is locked. */
  phoneNotes(id: string): Promise<NotesView>;
  /** Reply to, dismiss or open one of them on the phone. */
  phoneNoteAction(id: string, action: NoteAction): Promise<NoteOutcome>;
  /** The phone's per-app notification list, by name; read only here (it is changed on the phone). */
  phoneApps(id: string): Promise<NotifyAppRow[]>;
  /** Have the phone remind its user to grant what `feature` (notifications, messages) needs; whether it showed. */
  remindPhone(id: string, feature: Feature): Promise<boolean>;
  /**
   * A page of conversations after `before` (`null`: the newest), with the
   * provisional texts (`phone_sms` with `id` 0) the UI holds on top of their
   * threads, days counted on `clock`.
   */
  phoneThreads(
    id: string,
    before: Cursor | null,
    incoming: SmsMessage[],
    clock: LocalClock,
  ): Promise<ThreadsView>;
  /** A page of conversation `thread` after `before` (`null`: the newest), oldest first. */
  phoneThread(id: string, thread: number, before: Cursor | null): Promise<ThreadView>;
  /**
   * Text `to` from the phone, from SIM `sim` (`null`: the phone's default); sent at most once
   * per `clientId` (`unknown`: check the phone before resending).
   */
  sendSms(
    id: string,
    to: string,
    text: string,
    clientId: number,
    sim: number | null,
  ): Promise<SmsSendOutcome>;
  /** The SIMs the phone offers to text (`messages`) or call (`calls`) from; none with one SIM. */
  phoneSims(id: string, feature: Feature): Promise<SimChoice[]>;
  /** At most 10 of the phone's contact numbers whose name or number matches `query`. */
  phoneContacts(id: string, query: string): Promise<Contact[]>;
  /** Answer, decline, end, mute, dial, keypad tones or move the audio of the phone's call. */
  phoneCall(id: string, action: CallAction): Promise<CallOutcome>;
  /** A page of the phone's call log after `before` (`null`: the newest), by day on `clock`. */
  callHistory(id: string, before: Cursor | null, clock: LocalClock): Promise<CallLogView>;
  /** What the call windows show of `view` (`call_view` event) at `nowMs` (timer). */
  callWindow(view: CallView, nowMs: number): Promise<CallWindowView>;
  /** The last `call_view` event (`none` before any): a window opened mid-call starts from it. Desktop only. */
  callNow(): Promise<CallView>;
  /** How a linked phone shows here (Settings → Calls and Bluetooth). */
  phoneSettings(): Promise<PhoneSettings>;
  setPhoneSettings(settings: PhoneSettings): Promise<void>;
  /** The call-audio setup screen this computer's Bluetooth state calls for. */
  callAudioStep(): Promise<CallSetupStep>;
  /** Install the package call audio needs (the OS asks for the password; can take minutes). */
  installCallAudio(): Promise<void>;
  /** Turn this computer's hands-free role on, or off ("Set up again", audio off); only after the user agreed. */
  setCallAudioConfig(on: boolean): Promise<void>;
  /** On the phone: the step to show for permission `perm`. */
  phonePermStep(perm: PhonePerm): Promise<PermStep>;
  /** On the phone: ask for `perm` with Android's own dialog; the step it needs after the answer. */
  requestPhonePerm(perm: PhonePerm): Promise<PermStep>;
  /** On the phone: open the settings page that grants `perm`. */
  openPermSettings(perm: PhonePerm): Promise<void>;
  /** On the phone: which apps' notifications reach the linked computers, and the two filters. */
  notifyApps(): Promise<NotifyAppsView>;
  /** On the phone: turn app `appId`'s notifications on or off. */
  setNotifyApp(appId: string, on: boolean): Promise<void>;
  /** On the phone: hide one-time codes, show message text. */
  setNotifyFilters(hideCodes: boolean, showText: boolean): Promise<void>;
  /** On the phone: how far back texts reach the linked computers ("Keep in sync"). */
  messagesSync(): Promise<SyncWindow>;
  setMessagesSync(keep: SyncWindow): Promise<void>;

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
  /** Save the changed background fields (merged like {@link SettingsPatch}) and start at sign-in only while Keep running and Start at login are on. */
  setBackground(patch: Partial<BackgroundSettings>): Promise<void>;

  // --- App updates (never installed without the user's click) -------------
  checkUpdate(): Promise<UpdateStatus>;
  /**
   * Install exactly the version that `checkUpdate` offered; its steps arrive as `update` events.
   * A phone downloads on mobile data only if Settings or `mobileData` allow it.
   */
  installUpdate(version: string, mobileData?: boolean): Promise<UpdateStatus>;
  /** Stop the install in progress (it then fails with `cancelled`); nothing is installed. */
  cancelUpdate(): Promise<void>;
  /** Go on now: install while a move or call runs, or (a phone) download on mobile data. */
  installUpdateAnyway(): Promise<void>;
  /** Hold (`true`) or carry on with a phone's update download. */
  pauseUpdate(paused: boolean): Promise<void>;
  /** Open Android's "Install unknown apps" page for Carrova. */
  openInstallPermission(): Promise<void>;
  /** The version a phone was updated to since it last ran, once; `null` otherwise. */
  takeUpdated(): Promise<string | null>;
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
