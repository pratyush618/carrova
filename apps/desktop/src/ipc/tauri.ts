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
import { raise } from './reject';

/** The bridge's event channel (`carrova_view::ipc::EVENT_CHANNEL`). */
export const EVENT_CHANNEL = 'carrova://event';

/** `invoke`, rejecting only with a `BridgeError`. */
async function call<T>(command: string, args?: InvokeArgs): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (reason: unknown) {
    raise(toBridgeError(reason));
  }
}

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
    suggest: (session, preset) => call('suggest', { session, preset }),
    select: (session, selection, edits) => call('select', { session, selection, edits }),
    passphraseStrength: (passphrase) => call('passphrase_strength', { passphrase }),
    exportArchive: (session, path, passphrase, strength) =>
      call('export_archive', { session, path, passphrase, strength }),
    runSend: (session) => call('run_send', { session }),
    answerPairing: (session, offerNewCode) => call('answer_pairing', { session, offerNewCode }),

    openArchive: (path, passphrase) => call('open_archive', { path, passphrase }),
    connect: (code) => call('connect', { code }),
    cancelConnect: () => call('cancel_connect'),
    plan: (session, options) => call('plan', { session, options }),
    planOptions: (session) => call('plan_options', { session }),
    approve: (session) => call('approve', { session }),
    apply: (session) => call('apply', { session }),
    endReview: (session) => call('end_review', { session }),
    setTransferPaused: (session, paused) => call('set_transfer_paused', { session, paused }),
    ignoreFreeSpace: (session) => call('ignore_free_space', { session }),

    cancel: (session) => call('cancel', { session }),
    resume: (session, passphrase) => call('resume', { session, passphrase: passphrase ?? null }),
    sessions: () => call('sessions'),
    rollback: (session) => call('rollback', { session }),
    rollbackPreview: (session) => call('rollback_preview', { session }),
    dismiss: (session) => call('dismiss', { session }),

    catalog: () => call('catalog'),
    recipesStatus: () => call('recipes_status'),
    recipesUpdate: () => call('recipes_update'),
    recipesReset: () => call('recipes_reset'),

    about: () => call('about'),

    settings: () => call('settings'),
    saveSettings: (settings) => call('save_settings', { settings }),

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
