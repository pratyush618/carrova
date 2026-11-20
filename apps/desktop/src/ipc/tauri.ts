/**
 * `CarrovaPort` over the Tauri bridge: one `invoke` per method (the command
 * is the method name in snake_case, arguments by name) and the single
 * `carrova://event` channel. Rejections are normalised to `BridgeError`;
 * events are checked before they reach a listener.
 */
import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

import { isBridgeEvent, toBridgeError } from './guards';
import type { CarrovaPort } from './port';
import { bridgeError, raise } from './reject';

/** The bridge's event channel (`carrova_view::ipc::EVENT_CHANNEL`). */
export const EVENT_CHANNEL = 'carrova://event';

/** Opened links are waiting (`crate::opened::OPENED_EVENT`, no payload). */
const OPENED_EVENT = 'carrova://opened';

/** `invoke`, rejecting only with a `BridgeError`. */
async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason: unknown) {
    raise(toBridgeError(reason));
  }
}

/**
 * Code-less sends wait for ADR 0004's sign-off: the bridge has no such
 * commands yet, and `about().known_send` is false so the UI never calls these.
 */
const notYet = (): Promise<never> =>
  Promise.resolve().then(() =>
    raise(bridgeError('unsupported', 'Sending without a code is not available yet.')),
  );

/** The port backed by the running Tauri app. */
export function createTauriPort(): CarrovaPort {
  return {
    async subscribe(listener) {
      return listen<unknown>(EVENT_CHANNEL, ({ payload }) => {
        if (isBridgeEvent(payload)) {
          listener(payload);
        } else {
          console.warn('ignored a malformed bridge event');
        }
      });
    },

    scan: () => call('scan'),
    startManual: () => call('start_manual'),
    scanApps: (session) => call('scan_apps', { session }),
    pick: (session, selection, paths) => call('pick', { session, selection, paths }),
    takeSendRequest: () => call('take_send_request'),
    suggest: (session, preset) => call('suggest', { session, preset }),
    select: (session, selection, edits) => call('select', { session, selection, edits }),
    passphraseStrength: (passphrase) => call('passphrase_strength', { passphrase }),
    exportArchive: (session, path, passphrase, strength) =>
      call('export_archive', { session, path, passphrase, strength }),
    runSend: (session) => call('run_send', { session }),
    answerPairing: (session, offerNewCode) => call('answer_pairing', { session, offerNewCode }),
    requestSend: notYet,
    answerRequest: notYet,
    setSendRequests: notYet,

    openArchive: (path, passphrase) => call('open_archive', { path, passphrase }),
    connect: (code) => call('connect', { code }),
    cancelConnect: () => call('cancel_connect'),
    browseNearby: () => call('browse_nearby'),
    plan: (session, options) => call('plan', { session, options }),
    editPlan: (session, edits) => call('edit_plan', { session, edits }),
    planOptions: (session) => call('plan_options', { session }),
    approve: (session) => call('approve', { session }),
    apply: (session) => call('apply', { session }),
    endReview: (session) => call('end_review', { session }),
    setTransferPaused: (session, paused) => call('set_transfer_paused', { session, paused }),
    ignoreFreeSpace: (session) => call('ignore_free_space', { session }),
    retryConnection: (session) => call('retry_connection', { session }),

    cancel: (session) => call('cancel', { session }),
    resume: (session, passphrase) => call('resume', { session, passphrase: passphrase ?? null }),
    sessions: () => call('sessions'),
    rollback: (session) => call('rollback', { session }),
    rollbackPreview: (session) => call('rollback_preview', { session }),
    dismiss: (session) => call('dismiss', { session }),

    nearby: () => call('nearby'),
    knownDevices: () => call('known_devices'),
    renameDevice: (device, name) => call('rename_device', { device, name }),
    forgetDevice: (which) => call('forget_device', { which }),
    resetIdentity: () => call('reset_identity'),
    visibility: () => call('visibility'),
    setVisibility: (choice) => call('set_visibility', { choice }),
    wakeDevice: (device) => call('wake_device', { device }),
    takeShowNearby: () => call('take_show_nearby'),

    linkedDevices: () => call('linked_devices'),
    pairInvite: () => call('pair_invite'),
    answerPairPeer: (accept) => call('answer_pair_peer', { accept }),
    linkComputer: (ticket) => call('link_computer', { ticket }),
    linkInvite: (uri) => call('link_invite', { uri }),
    ownFingerprint: () => call('own_fingerprint'),
    lanAvailable: () => call('lan_available'),
    scanQr: () => call('scan_qr'),
    takeOpened: () => call('take_opened'),
    onOpened: (listener) =>
      listen(OPENED_EVENT, () => {
        listener();
      }),
    // `state` is taken by Tauri's managed state on the Rust side.
    setFeature: (id, feature, state) => call('set_feature', { id, feature, featureState: state }),
    answerAsk: (askId, accept) => call('answer_ask', { askId, accept }),
    sendClipboard: (id) => call('send_clipboard', { id }),
    sendFiles: (id, paths) => call('send_files', { id, paths }),
    handoffUrl: (id, url) => call('handoff_url', { id, url }),
    ring: (id) => call('ring', { id }),
    stopRing: (id) => call('stop_ring', { id }),
    sendText: (id, text) => call('send_text', { id, text }),
    clipHistory: () => call('clip_history'),
    clearClipHistory: () => call('clear_clip_history'),
    clipSettings: () => call('clip_settings'),
    setClipSettings: (settings) => call('set_clip_settings', { settings }),
    retryFile: (id) => call('retry_file', { id }),

    phoneNotes: (id) => call('phone_notes', { id }),
    phoneNoteAction: (id, action) => call('phone_note_action', { id, action }),
    phoneApps: (id) => call('phone_apps', { id }),
    remindPhone: (id, feature) => call('remind_phone', { id, feature }),
    phoneThreads: (id, before, incoming, clock) =>
      call('phone_threads', { id, before, incoming, clock }),
    phoneThread: (id, thread, before) => call('phone_thread', { id, thread, before }),
    sendSms: (id, to, text, clientId, sim) => call('send_sms', { id, to, text, clientId, sim }),
    phoneSims: (id, feature) => call('phone_sims', { id, feature }),
    phoneContacts: (id, query) => call('phone_contacts', { id, query }),
    phoneCall: (id, action) => call('phone_call', { id, action }),
    callHistory: (id, before, clock) => call('call_history', { id, before, clock }),
    callWindow: (view, nowMs) => call('call_window', { view, nowMs }),
    callNow: () => call('call_now'),
    phoneSettings: () => call('phone_settings'),
    setPhoneSettings: (settings) => call('set_phone_settings', { settings }),
    callAudioStep: () => call('call_audio_step'),
    installCallAudio: () => call('install_call_audio'),
    setCallAudioConfig: (on) => call('set_call_audio_config', { on }),
    phonePermStep: (perm) => call('phone_perm_step', { perm }),
    requestPhonePerm: (perm) => call('request_phone_perm', { perm }),
    openPermSettings: (perm) => call('open_perm_settings', { perm }),
    notifyApps: () => call('notify_apps'),
    setNotifyApp: (appId, on) => call('set_notify_app', { appId, on }),
    setNotifyFilters: (hideCodes, showText) => call('set_notify_filters', { hideCodes, showText }),
    messagesSync: () => call('messages_sync'),
    setMessagesSync: (keep) => call('set_messages_sync', { keep }),

    notices: () => call('notices'),
    markNoticesRead: (ids) => call('mark_notices_read', { ids: ids === 'all' ? null : ids }),

    catalog: () => call('catalog'),
    recipesStatus: () => call('recipes_status'),
    recipesCheck: () => call('recipes_check'),
    recipesUpdate: () => call('recipes_update'),
    recipesReset: () => call('recipes_reset'),

    about: () => call('about'),

    settings: () => call('settings'),
    saveSettings: (patch) => call('save_settings', { patch }),
    fileManagerStatus: () => call('file_manager_status'),
    setFileManagerEntries: (on) => call('set_file_manager_entries', { on }),
    setBackground: (patch) => call('set_background', { patch }),

    checkUpdate: () => call('check_update'),
    installUpdate: async (version) => {
      await call('install_update', { version });
    },
    restartApp: () => call('restart_app'),

    errorReportConsent: () => call('error_report_consent'),
    setErrorReportConsent: (consent) => call('set_error_report_consent', { consent }),
    errorReports: () => call('error_reports'),
    errorReport: (id) => call('error_report', { id }),
    deleteErrorReport: (id) => call('delete_error_report', { id }),
    clearErrorReports: () => call('clear_error_reports'),
    errorReportIssue: (id) => call('error_report_issue', { id }),
  };
}
