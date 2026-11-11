/** The sending side of the fake port: scan, selection, export, LAN send. */
import type { ArchiveSummary } from '../../bindings/ArchiveSummary';
import type { PassphraseStrength } from '../../bindings/PassphraseStrength';
import type { Picked } from '../../bindings/Picked';
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
import { addPicks, applyEdits, viewOf } from './selection';

/** The recorded suggestion; in the `plugin` scenario a plugin exports the Starship config. */
function suggested(scenario: Scenario): Selection {
  const { selection } = fx.scan;
  return scenario === 'plugin' ? { ...selection, export_plugins: [fx.PATH_REWRITE] } : selection;
}

const ROOTS = fx.scan.report.platform.roots;

/** The recorded scan with `selection` in place of its suggestion. */
function scanWith(selection: Selection): ScanResult {
  return { ...fx.scan, selection, view: viewOf(selection, fx.scan.view, ROOTS) };
}

/** A Manual session before the app scan: no apps, nothing picked. */
function manualStart(): ScanResult {
  const selection: Selection = { items: {}, export_plugins: [], confirmed_plugins: [] };
  const totals = {
    default_selected_size: 0,
    default_selected_files: 0,
    recognised_size: 0,
    unrecognised_size: 0,
    total_size: 0,
  };
  return {
    ...scanWith(selection),
    report: { ...fx.scan.report, apps: [], unrecognised: [], totals },
  };
}

/** The selection's picks, kept when the app scan's suggestion arrives. */
const picksOf = (selection: Selection): Selection['items'] =>
  Object.fromEntries(Object.entries(selection.items).filter(([, c]) => c.pick != null));

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
  /** The Manual picks the session holds (kept across its app scan). */
  private picked: Selection['items'] = {};

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

  /** Manual mode: a session without the app scan. */
  startManual(): Promise<ScanResult> {
    return later(() => {
      this.runtime.open(fx.exportSession);
      this.usable = true;
      this.picked = {};
      return manualStart();
    });
  }

  /** The app scan of a Manual session: the suggestion, with the session's picks kept. */
  async scanApps(session: SessionId): Promise<ScanResult> {
    this.runtime.slot(session);
    const scanned = await this.scan();
    const { selection } = scanned;
    const merged = scanWith({ ...selection, items: { ...selection.items, ...this.picked } });
    this.usable = merged.view.issues.length === 0;
    return merged;
  }

  /** Adds `paths` by the engine's pick rules, after a measuring step. */
  async pick(session: SessionId, selection: Selection, paths: string[]): Promise<Picked> {
    await this.runtime.step(session);
    const added = addPicks(selection, paths, ROOTS);
    const scan = scanWith(added.selection);
    this.picked = picksOf(added.selection);
    this.usable = scan.view.issues.length === 0;
    return { scan, outcomes: added.outcomes };
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
      this.picked = picksOf(result.selection);
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
