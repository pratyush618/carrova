import { OverrideDialog } from '../../screens/override/OverrideDialog';
import { OVERRIDES } from '../../screens/override/fixtures';
import type { OverrideModel } from '../../screens/override/model';
import { SendFoundScreen } from '../../screens/send/SendFoundScreen';
import { CustomizeSheet } from '../../screens/send/CustomizeSheet';
import { customize, foundReady } from '../../screens/send/fixtures';
import { RecvProgressScreen } from '../../screens/receive/RecvProgressScreen';
import { RecvReviewScreen } from '../../screens/receive/RecvReviewScreen';
import { ReviewDetailsSheet } from '../../screens/receive/ReviewDetailsSheet';
import {
  progressAt,
  reviewCrossOs,
  reviewDecide,
  reviewDetails,
} from '../../screens/receive/fixtures';
import type { View } from '../../app/nav/navMachine';
import { noops } from '../noops';
import { foundOn, transferOn } from './send';
import { reviewOn } from './receive';
import { states, type GalleryRender, type GalleryScreen } from '../types';

const overrideOn = noops('onKeepSafe', 'onOverride');
const dialog = (model: OverrideModel) => <OverrideDialog key="o" model={model} on={overrideOn} />;

/** Each override over the screen that raises it (mockup `OVERRIDES[*].base`). */
const BASES: Record<OverrideModel['kind'], GalleryRender> = {
  unmovable: {
    base: <SendFoundScreen model={foundReady} purpose="send" on={foundOn} />,
    overlays: [
      <CustomizeSheet
        key="c"
        model={customize}
        on={noops(
          'onFilter',
          'onToggleApp',
          'onToggleItem',
          'onTogglePath',
          'onExpand',
          'onIncludeAnyway',
          'onUnrecognised',
          'onIncludeCaches',
          'onReset',
          'onDone',
        )}
      />,
    ],
  },
  running: { base: <RecvReviewScreen model={reviewDecide} on={reviewOn} /> },
  space: {
    base: (
      <RecvProgressScreen
        model={progressAt('paused')}
        on={{ ...transferOn, ...noops('onRetry', 'onSeeWhat', 'onHome') }}
      />
    ),
  },
  cross_os: { base: <RecvReviewScreen model={reviewCrossOs} on={reviewOn} /> },
  elsewhere: {
    base: <RecvReviewScreen model={reviewDecide} on={reviewOn} />,
    overlays: [
      <ReviewDetailsSheet
        key="d"
        model={reviewDetails}
        on={noops('onRule', 'onFileRule', 'onRemap', 'onChoices', 'onDone')}
      />,
    ],
  },
};

const isKind = (s: string): s is OverrideModel['kind'] => s in OVERRIDES;

const VIEWS: Record<OverrideModel['kind'], View> = {
  unmovable: { kind: 'flow', at: { flow: 'send', step: 'found' } },
  running: { kind: 'flow', at: { flow: 'receive', step: 'review' } },
  space: { kind: 'flow', at: { flow: 'receive', step: 'progress' } },
  cross_os: { kind: 'flow', at: { flow: 'receive', step: 'review' } },
  elsewhere: { kind: 'flow', at: { flow: 'receive', step: 'review' } },
};

export const overrideScreens: readonly GalleryScreen[] = [
  {
    id: 'override',
    group: 'Overrides',
    title: 'Override warning (dialog)',
    view: VIEWS.unmovable,
    states: states(
      ['unmovable', 'Unmovable item'],
      ['running', 'App is running'],
      ['space', 'Free space'],
      ['cross_os', 'From another system'],
      ['elsewhere', 'Somewhere else'],
    ),
    render: (s) => {
      const kind = isKind(s) ? s : 'unmovable';
      const base = BASES[kind];
      return {
        base: base.base,
        view: VIEWS[kind],
        overlays: [...(base.overlays ?? []), dialog(OVERRIDES[kind])],
      };
    },
  },
];
