import type { View } from '../../app/nav/navMachine';
import { MappingSheet } from '../../screens/receive/MappingSheet';
import { PluginConsentDialog } from '../../screens/receive/PluginConsentDialog';
import { RecvCodeScreen } from '../../screens/receive/RecvCodeScreen';
import { RecvDoneScreen } from '../../screens/receive/RecvDoneScreen';
import { RecvProgressScreen } from '../../screens/receive/RecvProgressScreen';
import { RecvReviewScreen } from '../../screens/receive/RecvReviewScreen';
import { ReviewDetailsSheet } from '../../screens/receive/ReviewDetailsSheet';
import * as fx from '../../screens/receive/fixtures';
import type { CodeEntryModel, ReceiveProgressModel } from '../../screens/receive/model';
import { UndoConfirmDialog } from '../../screens/undo/UndoConfirmDialog';
import { UndoResultScreen } from '../../screens/undo/UndoResultScreen';
import { undoComplete, undoPartial, undoPreview } from '../../screens/undo/fixtures';
import { noops } from '../noops';
import { transferOn } from './send';
import { states, type GalleryScreen } from '../types';

const at = (step: 'code' | 'review' | 'progress' | 'done'): View => ({
  kind: 'flow',
  at: { flow: 'receive', step },
});

export const reviewOn = noops(
  'onAnswer',
  'onApplyWhileOpen',
  'onIgnoreSpace',
  'onBringAnyway',
  'onReviewPlugin',
  'onSeeWhere',
  'onSeeEvery',
  'onCancel',
  'onApprove',
);
const progressOn = { ...transferOn, ...noops('onRetry', 'onSeeWhat', 'onHome') };
const arrivedOn = noops('onRetry', 'onUndo', 'onDone');

const CODE: Record<string, CodeEntryModel> = {
  typing: fx.codeTyping,
  connecting: fx.codeConnecting,
  'not-found': fx.codeNotFound,
  mismatch: fx.codeMismatch,
};
const PROGRESS: Record<string, ReceiveProgressModel> = {
  moving: fx.progressAt('moving'),
  paused: fx.progressAt('paused'),
  held: fx.progressAt('held'),
  reconnecting: fx.progressAt('reconnecting'),
  applying: fx.progressAt('applying'),
  failed: fx.progressFailed,
};

export const reviewReady = <RecvReviewScreen model={fx.reviewReady} on={reviewOn} />;
export const reviewCrossOs = <RecvReviewScreen model={fx.reviewCrossOs} on={reviewOn} />;

export const receiveScreens: readonly GalleryScreen[] = [
  {
    id: 'recv-code',
    group: 'Receive',
    title: 'Type the code',
    view: at('code'),
    states: states(
      ['typing', 'Typing'],
      ['connecting', 'Connecting'],
      ['not-found', 'Not found'],
      ['mismatch', "Code didn't match"],
    ),
    render: (s) => ({
      base: (
        <RecvCodeScreen
          model={CODE[s] ?? fx.codeTyping}
          here={fx.ROUTE.to}
          on={noops('onChange', 'onAddress', 'onConnect', 'onCancel', 'onBack')}
        />
      ),
    }),
  },
  {
    id: 'recv-review',
    group: 'Receive',
    title: "Here's what will change",
    view: at('review'),
    states: states(
      ['decide', 'Needs answers'],
      ['ready', 'Ready'],
      ['cross-os', 'From another system'],
      ['mapping', 'Where things land (sheet)'],
    ),
    render: (s) => {
      switch (s) {
        case 'ready':
          return { base: reviewReady };
        case 'cross-os':
          return { base: reviewCrossOs };
        case 'mapping':
          return {
            base: reviewCrossOs,
            overlays: [<MappingSheet key="m" model={fx.mapping} onClose={() => undefined} />],
          };
        default:
          return { base: <RecvReviewScreen model={fx.reviewDecide} on={reviewOn} /> };
      }
    },
  },
  {
    id: 'plugin',
    group: 'Receive',
    title: 'Plugin permission consent (dialog)',
    view: at('review'),
    states: [],
    render: () => ({
      base: reviewCrossOs,
      overlays: [
        <PluginConsentDialog
          key="p"
          model={fx.pluginConsent}
          on={noops('onAllow', 'onDecline', 'onClose')}
        />,
      ],
    }),
  },
  {
    id: 'review-details',
    group: 'Receive',
    title: 'Every change (sheet)',
    view: at('review'),
    states: [],
    render: () => ({
      base: reviewReady,
      overlays: [
        <ReviewDetailsSheet
          key="d"
          model={fx.reviewDetails}
          on={noops('onRule', 'onFileRule', 'onRemap', 'onChoices', 'onDone')}
        />,
      ],
    }),
  },
  {
    id: 'recv-progress',
    group: 'Receive',
    title: 'Moving',
    view: at('progress'),
    states: states(
      ['moving', 'Moving'],
      ['paused', 'Disk full'],
      ['held', 'Paused by you'],
      ['reconnecting', 'Reconnecting'],
      ['applying', 'Putting in place'],
      ['failed', 'Failed, undone'],
    ),
    render: (s) => ({
      base: <RecvProgressScreen model={PROGRESS[s] ?? fx.progressAt('moving')} on={progressOn} />,
    }),
  },
  {
    id: 'recv-done',
    group: 'Receive',
    title: 'Arrived',
    view: at('done'),
    states: states(['done', 'All arrived'], ['partial', 'Some failed']),
    render: (s) => ({
      base: (
        <RecvDoneScreen
          model={s === 'partial' ? fx.arrivedPartial : fx.arrivedAll}
          on={arrivedOn}
        />
      ),
    }),
  },
  {
    id: 'undo-confirm',
    group: 'Receive',
    title: 'Undo preview (dialog)',
    view: at('done'),
    states: [],
    render: () => ({
      base: <RecvDoneScreen model={fx.arrivedAll} on={arrivedOn} />,
      overlays: [<UndoConfirmDialog key="u" model={undoPreview} on={noops('onKeep', 'onUndo')} />],
    }),
  },
  {
    id: 'undo-result',
    group: 'Receive',
    title: 'Undone',
    view: { kind: 'flow', at: { flow: 'undo', step: 'result' } },
    states: states(['complete', 'Complete'], ['partial', 'Partly failed']),
    render: (s) => ({
      base: (
        <UndoResultScreen
          model={s === 'partial' ? undoPartial : undoComplete}
          on={noops('onRetry', 'onOpenMovedAside', 'onDone')}
        />
      ),
    }),
  },
];
