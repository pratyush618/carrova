/**
 * The fake's linked phone on this computer: canned notifications, texts and
 * calls shaped like the engine's views (`carrova_view::phone`), the user's
 * phone settings, the call-audio setup, the phone's permission steps, its
 * per-app notification list and how far back its texts sync.
 * Tests needing other content set it through their own fixtures.
 */
import type { CallLogView } from '../../bindings/CallLogView';
import type { CallSetupStep } from '../../bindings/CallSetupStep';
import type { CallView } from '../../bindings/CallView';
import type { CallWindowView } from '../../bindings/CallWindowView';
import type { NotesView } from '../../bindings/NotesView';
import type { NotifyAppsView } from '../../bindings/NotifyAppsView';
import type { PermStep } from '../../bindings/PermStep';
import type { PhonePerm } from '../../bindings/PhonePerm';
import type { PhoneSettings } from '../../bindings/PhoneSettings';
import type { SyncWindow } from '../../bindings/SyncWindow';
import type { ThreadView } from '../../bindings/ThreadView';
import type { ThreadsView } from '../../bindings/ThreadsView';
import type { CarrovaPort } from '../port';
import { later } from './runtime';

type PhonePort = Pick<
  CarrovaPort,
  | 'phoneNotes'
  | 'phoneNoteAction'
  | 'phoneThreads'
  | 'phoneThread'
  | 'sendSms'
  | 'phoneCall'
  | 'callHistory'
  | 'callWindow'
  | 'phoneSettings'
  | 'setPhoneSettings'
  | 'callAudioStep'
  | 'installCallAudio'
  | 'setCallAudioConfig'
  | 'phonePermStep'
  | 'openPermSettings'
  | 'notifyApps'
  | 'setNotifyApp'
  | 'setNotifyFilters'
  | 'messagesSync'
  | 'setMessagesSync'
>;

const MINUTE = 60_000;

const notes = (now: number): NotesView => ({
  kind: 'list',
  notes: [
    {
      key: 'n1',
      app_id: 'org.chat',
      app_name: 'Chat',
      title: 'Sam Rivera',
      text: 'Are you still coming tonight?',
      when_ms: now - 2 * MINUTE,
      can_reply: true,
      icon: null,
      hidden: null,
    },
    {
      key: 'n2',
      app_id: 'org.bank',
      app_name: 'Bank of Example',
      title: '',
      text: '',
      when_ms: now - 30 * MINUTE,
      can_reply: false,
      icon: null,
      hidden: 'code',
    },
  ],
});

const threads = (now: number): ThreadsView => ({
  threads: [
    {
      id: 1,
      name: 'Sam Rivera',
      address: '+15550101',
      snippet: 'Are you still coming tonight?',
      when_ms: now - 2 * MINUTE,
      days_ago: 0,
      unread: 1,
    },
    {
      id: 2,
      name: null,
      address: '+15550142',
      snippet: 'See you then',
      when_ms: now - 26 * 60 * MINUTE,
      days_ago: 1,
      unread: 0,
    },
  ],
  next: null,
});

const thread = (now: number): ThreadView => ({
  messages: [
    { id: 10, outgoing: true, text: 'On my way', when_ms: now - 10 * MINUTE, provisional: false },
    {
      id: 11,
      outgoing: false,
      text: 'Are you still coming tonight?',
      when_ms: now - 2 * MINUTE,
      provisional: false,
    },
  ],
  next: null,
});

const history = (now: number): CallLogView => ({
  groups: [
    {
      days_ago: 0,
      rows: [
        {
          id: 3,
          caller: { number: '+15550101', name: 'Sam Rivera' },
          when_ms: now - 50 * MINUTE,
          duration_s: 240,
          kind: 'incoming',
        },
        {
          id: 2,
          caller: { number: '+15550101', name: 'Sam Rivera' },
          when_ms: now - 54 * MINUTE,
          duration_s: 0,
          kind: 'missed',
        },
      ],
    },
    {
      days_ago: 1,
      rows: [
        {
          id: 1,
          caller: { number: '+15550142', name: null },
          when_ms: now - 30 * 60 * MINUTE,
          duration_s: 60,
          kind: 'outgoing',
        },
      ],
    },
  ],
  next: null,
});

/** `mm:ss`, `h:mm:ss` from an hour, like the engine's call timer. */
function timer(elapsedMs: number): string {
  const secs = Math.floor(Math.max(0, elapsedMs) / 1000);
  const [h, m, s] = [Math.floor(secs / 3600), Math.floor(secs / 60) % 60, secs % 60];
  const two = (n: number) => String(n).padStart(2, '0');
  return h === 0 ? `${two(m)}:${two(s)}` : `${String(h)}:${two(m)}:${two(s)}`;
}

/** `carrova_view::phone::call_view`. */
function callWindow(view: CallView, nowMs: number): CallWindowView {
  switch (view.kind) {
    case 'none':
    case 'left':
      return { kind: 'closed' };
    case 'ringing':
      return {
        kind: 'ringing',
        device: view.device,
        caller: { number: view.number, name: view.name },
        ring_here: view.ring_here,
      };
    case 'active':
      return {
        kind: 'active',
        device: view.device,
        timer: timer(nowMs - view.since_ms),
        muted: view.muted,
        audio: view.audio,
        keypad: view.audio === 'laptop',
      };
    case 'missed':
      return {
        kind: 'missed',
        device: view.device,
        caller: { number: view.number, name: view.name },
      };
  }
}

/** Notification access is granted on a settings page; the rest by Android's prompt. */
const permStep = (perm: PhonePerm): PermStep =>
  perm === 'notification_access' ? 'open_settings' : 'ask';

/** The fake's phone commands: every action succeeds; call audio needs installing first. */
export function fakePhone(): PhonePort {
  let settings: PhoneSettings = {
    ring_here: 'while_active',
    only_while_active: true,
    show_when_locked: false,
  };
  let apps: NotifyAppsView = {
    apps: [
      { id: 'org.bank', name: 'Bank of Example', on: false, private: true },
      { id: 'org.chat', name: 'Chat', on: true, private: false },
      { id: 'org.mail', name: 'Mail', on: true, private: false },
    ],
    hide_codes: true,
    show_text: true,
  };
  let keep: SyncWindow = 'days30';
  let installed = false;
  let configured = false;
  const step = (): CallSetupStep => (installed && configured ? 'pair' : 'install');
  return {
    phoneNotes: () => later(() => notes(Date.now())),
    phoneNoteAction: () => later(() => 'done' as const),
    phoneThreads: () => later(() => threads(Date.now())),
    phoneThread: () => later(() => thread(Date.now())),
    sendSms: () => later(() => 'sent' as const),
    phoneCall: () => later(() => 'done' as const),
    callHistory: () => later(() => history(Date.now())),
    callWindow: (view, nowMs) => later(() => callWindow(view, nowMs)),
    phoneSettings: () => later(() => settings),
    setPhoneSettings: (next) =>
      later(() => {
        settings = next;
      }),
    callAudioStep: () => later(step),
    installCallAudio: () =>
      later(() => {
        installed = true;
      }),
    setCallAudioConfig: (on) =>
      later(() => {
        configured = on;
      }),
    phonePermStep: (perm) => later(() => permStep(perm)),
    openPermSettings: () => later(() => undefined),
    notifyApps: () => later(() => apps),
    setNotifyApp: (appId, on) =>
      later(() => {
        apps = { ...apps, apps: apps.apps.map((a) => (a.id === appId ? { ...a, on } : a)) };
      }),
    setNotifyFilters: (hideCodes, showText) =>
      later(() => {
        apps = { ...apps, hide_codes: hideCodes, show_text: showText };
      }),
    messagesSync: () => later(() => keep),
    setMessagesSync: (next) =>
      later(() => {
        keep = next;
      }),
  };
}
