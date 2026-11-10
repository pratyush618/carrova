import { useTranslation } from 'react-i18next';
import { useHost } from '../../../engine/host';
import { savedModel, saveLockModel } from '../../../flows/archive/saveFlow';
import { current } from '../../../flows/remote';
import {
  deliveredModel,
  foundModel,
  liveModel,
  pairingModel,
} from '../../../flows/send/sendProgressViews';
import { SaveDoneScreen } from '../../../screens/file/SaveDoneScreen';
import { SaveLockScreen } from '../../../screens/file/SaveLockScreen';
import { SendCodeScreen } from '../../../screens/send/SendCodeScreen';
import { SendDoneScreen } from '../../../screens/send/SendDoneScreen';
import { SendFoundScreen } from '../../../screens/send/SendFoundScreen';
import { SendLiveScreen } from '../../../screens/send/SendLiveScreen';
import type { FoundIntents, FoundPurpose } from '../../../screens/send/model';
import { FailedScreen } from '../../../screens/shared/StatusScreens';
import { useControllers } from '../../controllers/Controllers';
import type { FlowStep } from '../../nav/flows';
import { useSession } from '../../state/EngineState';
import { peerLabel } from '../../state/session';
import { useNav } from '../nav';

/** "Here's what we found", shared by sending and saving; only Continue differs. */
function FoundRoute({
  purpose,
  onContinue,
}: {
  readonly purpose: FoundPurpose;
  readonly onContinue: () => void;
}) {
  const { send } = useControllers();
  const nav = useNav();
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  if (send.state.scan.status === 'failed' && current(send.state.scan) === null) {
    return <FailedScreen error={send.state.scan.error} onRetry={send.start} onHome={home} />;
  }
  const on: FoundIntents = {
    onToggleApp: (app, included) => {
      send.edit({ kind: 'app', app, included });
    },
    onAnswer: (key, answer) => {
      send.edit({ kind: 'sensitive', key, bring: answer === 'bring' });
    },
    onCustomize: () => {
      nav.open({ kind: 'customize' });
    },
    onPickFolders: () => {
      nav.open({ kind: 'unrecognised' });
    },
    onSeeKnownApps: () => {
      nav.go({ type: 'openHub', page: 'apps' });
    },
    onContinue,
    onBack: home,
  };
  return <SendFoundScreen model={foundModel(send.state)} purpose={purpose} on={on} />;
}

/** The send flow: found → code → live → done. */
export function SendRoute({ step }: { readonly step: FlowStep<'send'> }) {
  const { t } = useTranslation();
  const { send } = useControllers();
  const nav = useNav();
  const progress = useSession(send.state.session);
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  switch (step) {
    case 'found':
      return <FoundRoute purpose="send" onContinue={send.send} />;
    case 'code':
      return (
        <SendCodeScreen
          model={pairingModel(send.state, progress)}
          on={{ onBack: send.back, onNewCode: send.newCode, onHome: home }}
        />
      );
    case 'live':
      return (
        <SendLiveScreen
          model={liveModel(send.state, progress, t('send.live.here'), t('history.anotherComputer'))}
          on={{ onCancel: home }}
        />
      );
    case 'done': {
      const report = current(send.state.run);
      if (report === null) return null;
      return (
        <SendDoneScreen
          model={deliveredModel(report, peerLabel(progress) ?? t('history.anotherComputer'))}
          on={{
            onDetails: () => {
              nav.go({ type: 'openHub', page: 'history' });
            },
            onDone: home,
          }}
        />
      );
    }
  }
}

/** Saving to a file: what goes in → passphrase and place → saved. */
export function SaveRoute({ step }: { readonly step: FlowStep<'save'> }) {
  const { save } = useControllers();
  const { revealPath } = useHost();
  const nav = useNav();
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  switch (step) {
    case 'what':
      return <FoundRoute purpose="save" onContinue={save.toLock} />;
    case 'lock':
      return (
        <SaveLockScreen
          model={saveLockModel(save.state, save.choosePath !== null)}
          on={{
            onPassphrase: save.setPassphrase,
            onConfirm: save.setConfirm,
            onPath: save.setPath,
            onKdf: save.setStrength,
            onChoosePath: save.choosePath ?? (() => undefined),
            onBack: save.back,
            onSave: save.save,
          }}
        />
      );
    case 'done': {
      const saved = savedModel(save.state);
      if (saved === null) return null;
      return (
        <SaveDoneScreen
          model={saved}
          on={{
            ...(revealPath && {
              onShowInFolder: () => {
                void revealPath(saved.path);
              },
            }),
            onDone: home,
          }}
        />
      );
    }
  }
}
