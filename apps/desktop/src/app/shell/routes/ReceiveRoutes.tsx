import { useTranslation } from 'react-i18next';
import { PAIRING_WORDS } from '../../../engine/wordlist/words';
import {
  codeEntryModel,
  openFileModel,
  receiveProgressModel,
} from '../../../flows/receive/progressViews';
import { arrivedModel, reviewModel } from '../../../flows/receive/reviewViews';
import { current } from '../../../flows/remote';
import { OpenFileScreen } from '../../../screens/file/OpenFileScreen';
import { RecvCodeScreen } from '../../../screens/receive/RecvCodeScreen';
import { RecvDoneScreen } from '../../../screens/receive/RecvDoneScreen';
import { RecvProgressScreen } from '../../../screens/receive/RecvProgressScreen';
import { RecvReviewScreen } from '../../../screens/receive/RecvReviewScreen';
import { BusyScreen, FailedScreen } from '../../../screens/shared/StatusScreens';
import { useControllers } from '../../controllers/Controllers';
import type { FlowStep } from '../../nav/flows';
import { useSession } from '../../state/EngineState';
import { useNav } from '../nav';

/** The other computer's name: the peer once connected, the file's name, or a neutral word. */
function usePeer(): string {
  const { t } = useTranslation();
  const { receive } = useControllers();
  const progress = useSession(receive.state.session);
  const { origin, file } = receive.state;
  if (progress.peer !== null) return progress.peer;
  if (origin === 'file' && file.path !== '') return file.path.slice(file.path.lastIndexOf('/') + 1);
  return t('history.anotherComputer');
}

function ReviewRoute() {
  const { t } = useTranslation();
  const { receive } = useControllers();
  const nav = useNav();
  const { review, options, answers } = receive.state;
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  const plan = current(review);
  if (plan === null) {
    return review.status === 'failed' ? (
      <FailedScreen error={review.error} onRetry={receive.retry} onHome={home} />
    ) : (
      <BusyScreen
        title={t('receive.review.title')}
        status={t('receive.review.planning')}
        onCancel={home}
      />
    );
  }
  return (
    <RecvReviewScreen
      model={reviewModel(plan, options, answers, review.status === 'loading')}
      on={{
        onAnswer: receive.answer,
        onApplyWhileOpen: (app) => {
          nav.open({ kind: 'override', reason: 'running', subject: app });
        },
        onIgnoreSpace: () => {
          nav.open({ kind: 'override', reason: 'space', subject: null });
        },
        onSeeEvery: () => {
          nav.open({ kind: 'review-details' });
        },
        onCancel: () => {
          receive.endReview();
          home();
        },
        onApprove: receive.approve,
      }}
    />
  );
}

/** Review, progress and done: shared by receiving over the LAN and opening a file. */
function ReceiveTail({ step }: { readonly step: 'review' | 'progress' | 'done' }) {
  const { receive, undo } = useControllers();
  const nav = useNav();
  const progress = useSession(receive.state.session);
  const peer = usePeer();
  const plan = current(receive.state.review);
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  switch (step) {
    case 'review':
      return <ReviewRoute />;
    case 'progress':
      return (
        <RecvProgressScreen
          model={receiveProgressModel(receive.state, progress, plan, peer)}
          on={{
            onCancel: home,
            ...(receive.state.origin === 'lan' && {
              onPause: () => {
                receive.setPaused(true);
              },
              onResume: () => {
                receive.setPaused(false);
              },
              onIgnoreSpace: () => {
                nav.open({ kind: 'override', reason: 'space', subject: null });
              },
            }),
            onRetry: receive.retry,
            onSeeWhat: () => {
              nav.go({ type: 'openHub', page: 'history' });
            },
            onHome: home,
          }}
        />
      );
    case 'done': {
      const checklist = current(receive.state.apply);
      const session = receive.state.session;
      if (checklist === null || session === null) return null;
      return (
        <RecvDoneScreen
          model={arrivedModel(checklist, plan, peer)}
          on={{
            onCopyCommand: (command) => {
              void navigator.clipboard.writeText(command);
            },
            ...(checklist.rollback_available && {
              onUndo: () => {
                undo.ask(session, new Date());
                nav.open({ kind: 'undo-confirm' });
              },
            }),
            onDone: home,
          }}
        />
      );
    }
  }
}

/** The receive flow: code → review → progress → done. */
export function ReceiveRoute({ step }: { readonly step: FlowStep<'receive'> }) {
  const { t } = useTranslation();
  const { receive } = useControllers();
  const nav = useNav();
  const peer = usePeer();
  if (step !== 'code') return <ReceiveTail step={step} />;
  return (
    <RecvCodeScreen
      model={codeEntryModel(receive.state.code, PAIRING_WORDS, t('send.live.here'), peer)}
      on={{
        onChange: receive.setCode,
        onAddress: receive.setAddress,
        onConnect: receive.open,
        onCancel: receive.abandon,
        onBack: () => {
          nav.go({ type: 'closeFlow' });
        },
      }}
    />
  );
}

/** Opening a file: file + passphrase, then the receive flow's screens. */
export function OpenRoute({ step }: { readonly step: FlowStep<'open'> }) {
  const { receive } = useControllers();
  const nav = useNav();
  if (step !== 'file') return <ReceiveTail step={step} />;
  return (
    <OpenFileScreen
      model={openFileModel(receive.state.file, receive.choosePath !== null)}
      on={{
        onPath: receive.setPath,
        onChooseAnother: receive.choosePath ?? (() => undefined),
        onPassphrase: receive.setPassphrase,
        onBack: () => {
          nav.go({ type: 'closeFlow' });
        },
        onOpen: receive.open,
      }}
    />
  );
}
