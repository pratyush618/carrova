import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useHost } from '../../../engine/host';
import { savedModel, saveLockModel } from '../../../flows/archive/saveFlow';
import { current } from '../../../flows/remote';
import {
  deliveredModel,
  foundModel,
  linkedModel,
  liveModel,
  modeSwitch,
  pairingModel,
  sendMove,
} from '../../../flows/send/sendProgressViews';
import { pickSecret } from '../../../flows/send/sendViews';
import type { ItemKey } from '../../../engine/types';
import { SaveDoneScreen } from '../../../screens/file/SaveDoneScreen';
import { SaveLockScreen } from '../../../screens/file/SaveLockScreen';
import { SendCodeScreen } from '../../../screens/send/SendCodeScreen';
import { SendDoneScreen } from '../../../screens/send/SendDoneScreen';
import { PluginConsentDialog } from '../../../screens/receive/PluginConsentDialog';
import { KeyConfirmDialog } from '../../../screens/send/KeyConfirmDialog';
import { SendFoundScreen } from '../../../screens/send/SendFoundScreen';
import { SendLinkedScreen } from '../../../screens/send/SendLinkedScreen';
import { SendLiveScreen } from '../../../screens/send/SendLiveScreen';
import type { FoundFilter, FoundIntents, FoundPurpose } from '../../../screens/send/model';
import { FailedScreen } from '../../../screens/shared/StatusScreens';
import type { RouteEnd } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import type { FlowStep } from '../../nav/flows';
import { useSession } from '../../state/EngineState';
import { peerLabel } from '../../state/session';
import { useNav } from '../nav';

/** This computer as the route chip and rail show it; `null` until the engine has said. */
function useHere(): RouteEnd | null {
  const { t } = useTranslation();
  const about = useControllers().settings.about;
  return about === null ? null : { os: about.os, name: about.device_name ?? t('send.live.here') };
}

/** "Here's what we found", shared by sending and saving; only Continue differs. */
function FoundRoute({
  purpose,
  onContinue,
}: {
  readonly purpose: FoundPurpose;
  readonly onContinue: () => void;
}) {
  const { send, save } = useControllers();
  const nav = useNav();
  const here = useHere();
  const [confirming, setConfirming] = useState<ItemKey | null>(null);
  const [consenting, setConsenting] = useState<string | null>(null);
  const [filter, setFilter] = useState('');
  const [foundFilter, setFoundFilter] = useState<FoundFilter>('all');
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  if (send.state.scan.status === 'failed' && current(send.state.scan) === null) {
    return <FailedScreen error={send.state.scan.error} onRetry={send.retryScan} onHome={home} />;
  }
  const model = foundModel(send.state, filter, foundFilter);
  const pick =
    model.status === 'manual' ? model.picks.find((p) => p.key === confirming) : undefined;
  const secret =
    model.status === 'found'
      ? model.secrets.find((s) => s.key === confirming)
      : pick && pickSecret(pick);
  const consent =
    model.status === 'found' ? model.plugins.find((p) => p.plugin === consenting) : undefined;
  const answer = (plugin: string, allow: boolean) => {
    send.edit({ kind: 'plugin', plugin, allow });
    setConsenting(null);
  };
  const bring = (key: ItemKey, yes: boolean) => {
    send.edit({ kind: 'sensitive', key, bring: yes });
  };
  const on: FoundIntents = {
    onToggleApp: (app, included) => {
      send.edit({ kind: 'app', app, included });
    },
    onConfirmKey: setConfirming,
    onReviewPlugin: setConsenting,
    onLeaveKey: (key) => {
      bring(key, false);
    },
    onFilter: setFilter,
    onFoundFilter: setFoundFilter,
    onPreset: (preset) => {
      send.edit({ kind: 'preset', preset });
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
    ...(purpose === 'send' && {
      onSaveInstead: () => {
        save.startFromSelection();
        nav.go({ type: 'enterFlow', at: { flow: 'save', step: 'what' } });
      },
    }),
    onContinue,
    onBack: home,
    onMode: (manual) => {
      send.switchMode(manual ? 'manual' : 'auto');
    },
    ...(send.addFiles && { onAddFiles: send.addFiles }),
    ...(send.addFolders && { onAddFolders: send.addFolders }),
    onRemovePick: send.removePick,
  };
  const close = () => {
    setConfirming(null);
  };
  return (
    <>
      <SendFoundScreen
        model={model}
        purpose={purpose}
        here={here}
        mode={modeSwitch(send.state)}
        on={on}
      />
      {confirming !== null && secret && (
        <KeyConfirmDialog
          secret={secret}
          on={{
            onBring: () => {
              bring(confirming, true);
              close();
            },
            onLeave: close,
          }}
        />
      )}
      {consent && (
        <PluginConsentDialog
          model={consent}
          on={{
            onAllow: () => {
              answer(consent.plugin, true);
            },
            onDecline: () => {
              answer(consent.plugin, false);
            },
            onClose: () => {
              setConsenting(null);
            },
          }}
        />
      )}
    </>
  );
}

/** The send flow: found → code → linked, then live → done. */
export function SendRoute({ step }: { readonly step: FlowStep<'send'> }) {
  const { t } = useTranslation();
  const { send } = useControllers();
  const nav = useNav();
  const here = useHere();
  const progress = useSession(send.state.session);
  const home = () => {
    nav.go({ type: 'closeFlow' });
  };
  const peer = t('history.anotherComputer');
  switch (step) {
    case 'found':
      return <FoundRoute purpose="send" onContinue={send.send} />;
    case 'code':
      return (
        <SendCodeScreen
          model={pairingModel(send.state, progress)}
          move={sendMove(send.state, here)}
          on={{ onBack: send.back, onNewCode: send.newCode, onHome: home }}
        />
      );
    case 'live': {
      const live = liveModel(send.state, progress, t('send.live.here'), peer);
      return live.status === 'reviewing' ? (
        <SendLinkedScreen
          model={linkedModel(send.state, progress, here, peer)}
          on={{ onCancel: home }}
        />
      ) : (
        <SendLiveScreen model={live} on={{ onCancel: home }} />
      );
    }
    case 'done': {
      const report = current(send.state.run);
      if (report === null) return null;
      return (
        <SendDoneScreen
          model={deliveredModel(report, peerLabel(progress) ?? peer)}
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
