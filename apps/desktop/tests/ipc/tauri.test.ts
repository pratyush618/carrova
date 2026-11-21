import { emit } from '@tauri-apps/api/event';
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { BridgeError } from '@/bindings/BridgeError';
import type { BridgeEvent } from '@/bindings/BridgeEvent';
import { scan } from '@/ipc/fixtures/scan';
import { raise } from '@/ipc/reject';
import { createTauriPort, EVENT_CHANNEL } from '@/ipc/tauri';

afterEach(() => {
  clearMocks();
});

const session = scan.session;

describe('the Tauri port', () => {
  it('sends a feature change as `featureState` and an answer as `askId`', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return null;
    });
    const port = createTauriPort();
    await port.setFeature('p8', 'files', 'off');
    await port.answerAsk(7, true);
    await port.answerPairPeer(false);
    await port.linkInvite('carrova://pair?v=1&t=4-maple-orbit&k=KEY');
    await port.ownFingerprint();
    expect(calls).toEqual([
      ['set_feature', { id: 'p8', feature: 'files', featureState: 'off' }],
      ['answer_ask', { askId: 7, accept: true }],
      ['answer_pair_peer', { accept: false }],
      ['link_invite', { uri: 'carrova://pair?v=1&t=4-maple-orbit&k=KEY' }],
      ['own_fingerprint', {}],
    ]);
  });

  it('sends the linked-phone commands with their arguments by name', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return null;
    });
    const port = createTauriPort();
    const clock = { now_ms: 1_000, utc_offset_min: 60 };
    const before = { when_ms: 5, id: 7 };
    await port.phoneNotes('p8');
    await port.phoneNoteAction('p8', { kind: 'dismiss', key: 'k' });
    await port.phoneApps('p8');
    await port.remindPhone('p8', 'messages');
    await port.phoneThreads('p8', null, [], clock);
    await port.phoneThread('p8', 3, before);
    await port.sendSms('p8', '+15550101', 'hi', 42, 5);
    await port.phoneSims('p8', 'calls');
    await port.phoneCall('p8', { kind: 'mute', on: true });
    await port.callHistory('p8', before, clock);
    await port.callWindow({ kind: 'none' }, 9);
    await port.callNow();
    await port.phoneSettings();
    await port.setPhoneSettings({
      ring_here: 'never',
      only_while_active: false,
      show_when_locked: false,
    });
    await port.callAudioStep();
    await port.installCallAudio();
    await port.setCallAudioConfig(false);
    await port.phonePermStep('sms');
    await port.requestPhonePerm('calls');
    await port.openPermSettings('notification_access');
    expect(calls).toEqual([
      ['phone_notes', { id: 'p8' }],
      ['phone_note_action', { id: 'p8', action: { kind: 'dismiss', key: 'k' } }],
      ['phone_apps', { id: 'p8' }],
      ['remind_phone', { id: 'p8', feature: 'messages' }],
      ['phone_threads', { id: 'p8', before: null, incoming: [], clock }],
      ['phone_thread', { id: 'p8', thread: 3, before }],
      ['send_sms', { id: 'p8', to: '+15550101', text: 'hi', clientId: 42, sim: 5 }],
      ['phone_sims', { id: 'p8', feature: 'calls' }],
      ['phone_call', { id: 'p8', action: { kind: 'mute', on: true } }],
      ['call_history', { id: 'p8', before, clock }],
      ['call_window', { view: { kind: 'none' }, nowMs: 9 }],
      ['call_now', {}],
      ['phone_settings', {}],
      [
        'set_phone_settings',
        { settings: { ring_here: 'never', only_while_active: false, show_when_locked: false } },
      ],
      ['call_audio_step', {}],
      ['install_call_audio', {}],
      ['set_call_audio_config', { on: false }],
      ['phone_perm_step', { perm: 'sms' }],
      ['request_phone_perm', { perm: 'calls' }],
      ['open_perm_settings', { perm: 'notification_access' }],
    ]);
  });

  it('asks the phone commands by name: LAN, QR scan and opened links', async () => {
    const calls: string[] = [];
    mockIPC((command) => {
      calls.push(command);
      return command === 'lan_available' ? false : null;
    });
    const port = createTauriPort();
    await expect(port.lanAvailable()).resolves.toBe(false);
    await port.scanQr();
    await port.takeOpened();
    expect(calls).toEqual(['lan_available', 'scan_qr', 'take_opened']);
  });

  it('invokes the snake_case command with named arguments', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return command === 'scan' ? scan : null;
    });
    const port = createTauriPort();

    await expect(port.scan()).resolves.toEqual(scan);
    await port.select(session, scan.selection, [{ kind: 'caches', included: true }]);
    await port.passphraseStrength('orbit lemon');
    await port.planOptions(session);
    await port.answerPairing(session, true);
    await port.exportArchive(session, '/tmp/a.carrova', 'pass', 'strong');
    await port.resume(session);
    await port.cancelConnect();
    await port.startManual();
    await port.scanApps(session);
    await port.pick(session, scan.selection, ['/home/ana/notes']);
    await port.takeSendRequest();
    await port.fileManagerStatus();
    await port.setFileManagerEntries(true);
    await port.notices();
    await port.markNoticesRead([4, 5]);
    await port.markNoticesRead('all');
    await port.setBackground({ lean: false });
    await port.saveSettings({ theme: null });

    expect(calls).toEqual([
      ['scan', {}],
      [
        'select',
        { session, selection: scan.selection, edits: [{ kind: 'caches', included: true }] },
      ],
      ['passphrase_strength', { passphrase: 'orbit lemon' }],
      ['plan_options', { session }],
      ['answer_pairing', { session, offerNewCode: true }],
      [
        'export_archive',
        { session, path: '/tmp/a.carrova', passphrase: 'pass', strength: 'strong' },
      ],
      ['resume', { session, passphrase: null }],
      ['cancel_connect', {}],
      ['start_manual', {}],
      ['scan_apps', { session }],
      ['pick', { session, selection: scan.selection, paths: ['/home/ana/notes'] }],
      ['take_send_request', {}],
      ['file_manager_status', {}],
      ['set_file_manager_entries', { on: true }],
      ['notices', {}],
      ['mark_notices_read', { ids: [4, 5] }],
      ['mark_notices_read', { ids: null }],
      ['set_background', { patch: { lean: false } }],
      ['save_settings', { patch: { theme: null } }],
    ]);
  });

  it('names the device commands as the bridge registers them', async () => {
    const calls: [string, unknown][] = [];
    mockIPC((command, args) => {
      calls.push([command, args]);
      return null;
    });
    const port = createTauriPort();
    await port.nearby();
    await port.knownDevices();
    await port.renameDevice('7F3A·19C2', 'Ana at work');
    await port.forgetDevice({ device: '7F3A·19C2' });
    await port.forgetDevice('all');
    await port.resetIdentity();
    await port.visibility();
    await port.setVisibility('everyone');
    await port.wakeDevice('7F3A·19C2');
    await port.takeShowNearby();
    expect(calls).toEqual([
      ['nearby', {}],
      ['known_devices', {}],
      ['rename_device', { device: '7F3A·19C2', name: 'Ana at work' }],
      ['forget_device', { which: { device: '7F3A·19C2' } }],
      ['forget_device', { which: 'all' }],
      ['reset_identity', {}],
      ['visibility', {}],
      ['set_visibility', { choice: 'everyone' }],
      ['wake_device', { device: '7F3A·19C2' }],
      ['take_show_nearby', {}],
    ]);
  });

  it('refuses code-less sends without calling the bridge (not available yet)', async () => {
    const calls: string[] = [];
    mockIPC((command) => {
      calls.push(command);
      return null;
    });
    const port = createTauriPort();
    await expect(port.requestSend(session, 'd2')).rejects.toMatchObject({ code: 'unsupported' });
    await expect(port.answerRequest(1, 'review')).rejects.toMatchObject({ code: 'unsupported' });
    await expect(port.setSendRequests('d2', false)).rejects.toMatchObject({
      code: 'unsupported',
    });
    expect(calls).toEqual([]);
  });

  it('rejects with the bridge error the command failed with', async () => {
    const error: BridgeError = { code: 'blocked', message: 'plan is blocked', hint: 'Close it.' };
    mockIPC(() => raise(error));
    await expect(createTauriPort().approve(session)).rejects.toEqual(error);
  });

  it('normalises any other failure into an internal bridge error', async () => {
    mockIPC(() => {
      throw new Error('IPC is down');
    });
    await expect(createTauriPort().sessions()).rejects.toEqual({
      code: 'internal',
      message: 'IPC is down',
      hint: null,
    });
  });

  it('delivers well-formed events and drops malformed ones', async () => {
    mockIPC(() => null, { shouldMockEvents: true });
    const listener = vi.fn<(event: BridgeEvent) => void>();
    const unsubscribe = await createTauriPort().subscribe(listener);
    const good: BridgeEvent = { kind: 'task_ended', session, error: null };
    const nudge: BridgeEvent = { kind: 'send_requested' };

    await emit(EVENT_CHANNEL, good);
    await emit(EVENT_CHANNEL, nudge);
    await emit(EVENT_CHANNEL, { kind: 'engine', session, event: 'not an event' });
    await emit(EVENT_CHANNEL, { kind: 'mystery', session });
    await emit(EVENT_CHANNEL, { kind: 'task_ended', session, error: { code: 'nope' } });

    expect(listener.mock.calls).toEqual([[good], [nudge]]);
    unsubscribe();
  });
});
