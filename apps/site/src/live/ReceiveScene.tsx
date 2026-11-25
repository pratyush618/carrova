import { useCallback, useEffect, useState } from 'react';
import { RecvDoneScreen } from '../../../desktop/src/screens/receive/RecvDoneScreen';
import { RecvProgressScreen } from '../../../desktop/src/screens/receive/RecvProgressScreen';
import { RecvReviewScreen } from '../../../desktop/src/screens/receive/RecvReviewScreen';
import * as fx from '../../../desktop/src/screens/receive/fixtures';
import type {
  ArrivedModel,
  DecisionModel,
  ReceiveProgressModel,
  ReviewModel,
} from '../../../desktop/src/screens/receive/model';
import { APP_ORDER, MB } from '../../../desktop/src/screens/shared/fixtures';
import type { TransferApp } from '../../../desktop/src/screens/shared/transfer';
import { transferAt } from '../../../desktop/src/screens/shared/transferFixtures';
import { AppWindow, flowAt } from './AppWindow';

const ignore = (): void => undefined;

/** How long the demo move takes on screen, and what it pretends to be. */
const MOVE_MS = 7000;
const PRETEND_SECONDS = 120;
const APPLY_MS = 1400;
const TICK_MS = 80;

type Stage =
  | { readonly at: 'review' }
  | { readonly at: 'move'; readonly since: number }
  | { readonly at: 'done' };

/** The review a real receive shows, with the answers given so far. */
export function reviewWith(decisions: readonly DecisionModel[]): ReviewModel {
  const keys = decisions.flatMap((d) =>
    d.kind === 'secret' && d.answer === 'bring' ? [d.title] : [],
  );
  return {
    ...fx.reviewDecide,
    decisions,
    blocked: decisions.some((d) => d.answer === null),
    rail: { ...fx.reviewDecide.rail, keys: keys.length > 0 ? keys : null },
  };
}

/** Apps the visitor chose to skip ("Skip VS Code"), by name. */
export function skippedApps(decisions: readonly DecisionModel[]): string[] {
  return decisions.flatMap((d) => (d.kind === 'app-running' && d.answer === 'skip' ? [d.app] : []));
}

/** The transfer `share` (0–1) of the way through, without `skipped` apps: they arrive in order, by size. */
export function movingAt(share: number, skipped: readonly string[] = []): ReceiveProgressModel {
  const order = APP_ORDER.filter((a) => !skipped.includes(a.name));
  const total = order.reduce((sum, a) => sum + a.bytes, 0);
  const arrived = share * total;
  let before = 0;
  const apps: TransferApp[] = order.map((app) => {
    const start = before;
    before += app.bytes;
    if (arrived >= before) return { ...app, state: 'done', bytesDone: 0 };
    if (arrived > start) return { ...app, state: 'moving', bytesDone: arrived - start };
    return { ...app, state: 'waiting', bytesDone: 0 };
  });
  const applying = transferAt('applying', 'receive');
  return {
    status: 'transfer',
    transfer:
      share >= 1
        ? {
            ...applying,
            done: total,
            total,
            apps: applying.apps.filter((a) => !skipped.includes(a.name)),
          }
        : {
            ...transferAt('moving', 'receive'),
            done: arrived,
            total,
            secondsLeft: Math.max(1, Math.round((1 - share) * PRETEND_SECONDS)),
            bytesPerSecond: 10 * MB,
            apps,
          },
    resuming: false,
    resumeError: null,
  };
}

/** The receipt for a move without `skipped` apps: the same apps the transfer carried. */
export function arrivedWithout(skipped: readonly string[]): ArrivedModel {
  const carried = APP_ORDER.filter((a) => !skipped.includes(a.name));
  const names = carried.map((a) => a.name);
  return {
    ...fx.arrivedAll,
    moved: names,
    appsTotal: names.length,
    toInstall: fx.arrivedAll.toInstall.filter((name) => names.includes(name)),
    bytes: carried.reduce((sum, a) => sum + a.bytes, 0),
  };
}

/** Milliseconds since the move began, ticking while it runs; `onEnd` once it has run `ms`. */
function useElapsed(since: number | null, ms: number, onEnd: () => void): number {
  const [now, setNow] = useState(() => performance.now());
  useEffect(() => {
    if (since === null) return undefined;
    const timer = window.setInterval(() => {
      const t = performance.now();
      setNow(t);
      if (t - since >= ms) onEnd();
    }, TICK_MS);
    return () => {
      window.clearInterval(timer);
    };
  }, [since, ms, onEnd]);
  return since === null ? 0 : Math.max(0, now - since);
}

/**
 * Receive, end to end, with the app's own screens: answer the questions,
 * approve, watch it move, and land on "Everything arrived".
 */
export function ReceiveScene() {
  const [stage, setStage] = useState<Stage>({ at: 'review' });
  const [decisions, setDecisions] = useState(fx.reviewDecide.decisions);
  const finish = useCallback(() => {
    setStage({ at: 'done' });
  }, []);
  const elapsed = useElapsed(stage.at === 'move' ? stage.since : null, MOVE_MS + APPLY_MS, finish);
  const reset = () => {
    setDecisions(fx.reviewDecide.decisions);
    setStage({ at: 'review' });
  };
  const answer = (key: string, value: DecisionModel['answer']) => {
    setDecisions((ds) =>
      ds.map((d) => (d.key === key ? ({ ...d, answer: value } as DecisionModel) : d)),
    );
  };

  if (stage.at === 'done') {
    return (
      <AppWindow view={flowAt('receive', 'done')}>
        <RecvDoneScreen
          model={arrivedWithout(skippedApps(decisions))}
          on={{ onDone: reset, onUndo: reset }}
        />
      </AppWindow>
    );
  }
  if (stage.at === 'move') {
    return (
      <AppWindow view={flowAt('receive', 'progress')}>
        <RecvProgressScreen
          model={movingAt(Math.min(1, elapsed / MOVE_MS), skippedApps(decisions))}
          on={{ onCancel: reset, onSeeWhat: ignore, onHome: reset }}
        />
      </AppWindow>
    );
  }
  return (
    <AppWindow view={flowAt('receive', 'review')}>
      <RecvReviewScreen
        model={reviewWith(decisions)}
        on={{
          onAnswer: answer,
          onAcceptSafePicks: ignore,
          onApplyWhileOpen: (key) => {
            answer(key, 'closed');
          },
          onIgnoreSpace: ignore,
          onBringAnyway: ignore,
          onReviewPlugin: ignore,
          onSeeWhere: ignore,
          onSeeEvery: ignore,
          onCancel: reset,
          onApprove: () => {
            setStage({ at: 'move', since: performance.now() });
          },
        }}
      />
    </AppWindow>
  );
}
