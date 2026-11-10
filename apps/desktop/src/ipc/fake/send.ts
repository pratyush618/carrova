/** The sending side of the fake port: scan, selection, export, LAN send. */
import type { ArchiveSummary } from '../../bindings/ArchiveSummary';
import type { PassphraseStrength } from '../../bindings/PassphraseStrength';
import type { Preset } from '../../bindings/Preset';
import type { ScanResult } from '../../bindings/ScanResult';
import type { Selection } from '../../bindings/Selection';
import type { SelectionEdit } from '../../bindings/SelectionEdit';
import type { SendReport } from '../../bindings/SendReport';
import type { SessionId } from '../../bindings/SessionId';
import { bridgeError, raise } from '../reject';
import * as fx from './fixtures';
import { later, type Runtime } from './runtime';
import type { Scenario } from './scenario';
import { applyEdits, viewOf } from './selection';

/** The recorded suggestion; in the `plugin` scenario a plugin exports the Starship config. */
function suggested(scenario: Scenario): Selection {
  const { selection } = fx.scan;
  return scenario === 'plugin' ? { ...selection, export_plugins: [fx.PATH_REWRITE] } : selection;
}

/** The recorded scan with `selection` in place of its suggestion. */
function scanWith(selection: Selection): ScanResult {
  return { ...fx.scan, selection, view: viewOf(selection, fx.scan.view) };
}

/**
 * The fake's passphrase meter, by length only (a test convention, not the
 * engine's estimate): under 10 characters is weak, then fair, good from 16,
 * strong from 24.
 */
function strengthOf(passphrase: string): PassphraseStrength {
  const n = passphrase.length;
  if (n >= 24) return 'strong';
  if (n >= 16) return 'good';
  return n >= 10 ? 'fair' : 'weak';
}

/** Sending-side flows over `runtime`. */
export class FakeSender {
  /** Whether the latest selection was usable (export and send need it). */
  private usable = true;

  constructor(
    private readonly runtime: Runtime,
    private readonly scenario: Scenario,
  ) {}

  async scan(): Promise<ScanResult> {
    if (this.scenario === 'no-recipes') {
      raise(bridgeError('recipes', 'no recipes available', 'Update recipes in Settings.'));
    }
    this.runtime.open(fx.exportSession);
    await this.runtime.step(fx.exportSession, 2);
    if (this.scenario !== 'plugin') {
      this.usable = true;
      return fx.scan;
    }
    this.usable = false;
    return scanWith(suggested(this.scenario));
  }

  suggest(session: SessionId, preset: Preset): Promise<ScanResult> {
    return later(() => {
      this.runtime.slot(session);
      return scanWith(applyEdits(suggested(this.scenario), [{ kind: 'preset', preset }]));
    });
  }

  select(session: SessionId, selection: Selection, edits: SelectionEdit[]): Promise<ScanResult> {
    return later(() => {
      this.runtime.slot(session);
      const result = scanWith(applyEdits(selection, edits));
      this.usable = result.view.issues.length === 0;
      return result;
    });
  }

  passphraseStrength(passphrase: string): Promise<PassphraseStrength> {
    return later(() => strengthOf(passphrase));
  }

  private ensureUsable(): void {
    if (!this.usable) {
      raise(bridgeError('invalid_selection', 'the selection has issues to resolve first'));
    }
  }

  exportArchive(session: SessionId, passphrase: string): Promise<ArchiveSummary> {
    return this.runtime.task(session, async () => {
      this.ensureUsable();
      if (strengthOf(passphrase) === 'weak') {
        raise(
          bridgeError('wrong_passphrase', 'the passphrase is weak; an archive needs a fair one'),
        );
      }
      for (const { event } of fx.exportEvents.filter((e) => e.kind === 'engine')) {
        await this.runtime.step(session);
        this.runtime.engine(session, event);
      }
      return fx.archiveSummary;
    });
  }

  runSend(session: SessionId): Promise<SendReport> {
    return this.runtime.task(session, async () => {
      this.ensureUsable();
      await this.pair(session);
      const rt = this.runtime;
      rt.engine(session, { event: 'peer_connected', peer: fx.peerAddress });
      await rt.step(session);
      rt.engine(session, { event: 'inventory_received', ...fx.inventory });
      await rt.step(session, 2);
      rt.engine(session, { event: 'plan_summary_received', ...fx.planSummary });
      const total = fx.archiveSummary.bytes;
      for (let done = 0; done <= total; done += Math.ceil(total / 4)) {
        await rt.step(session);
        rt.engine(session, { event: 'progress', phase: 'send', done, total });
        if (this.scenario === 'reconnect' && done === 0) {
          rt.engine(session, { event: 'peer_lost', retrying: true });
          await rt.step(session, 3);
          rt.engine(session, { event: 'peer_connected', peer: fx.peerAddress });
        }
      }
      rt.engine(session, { event: 'finished', checklist: fx.checklist });
      return { checklist: fx.checklist, chunks: 9, bytes: total, outcome: 'applied' };
    });
  }

  /** Show the code; in `wrong-code`, a failed attempt first asks for a new one. */
  private async pair(session: SessionId): Promise<void> {
    const rt = this.runtime;
    rt.emit({ kind: 'pairing_code', session, code: fx.codeView(false) });
    await rt.step(session, 3);
    if (this.scenario !== 'wrong-code') {
      return;
    }
    rt.engine(session, { event: 'pairing_failed' });
    const answer = rt.question(session);
    rt.emit({ kind: 'confirm_new_code', session, failures: 1, remaining: 2 });
    const offerNewCode = await answer;
    await rt.step(session, 0);
    if (!offerNewCode) {
      raise(
        bridgeError(
          'pairing_failed',
          'the send was stopped after 1 wrong code(s); no new code was offered',
        ),
      );
    }
    rt.emit({ kind: 'pairing_code', session, code: fx.codeView(true) });
    await rt.step(session, 3);
  }
}
