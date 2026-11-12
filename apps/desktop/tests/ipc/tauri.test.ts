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
    ]);
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
