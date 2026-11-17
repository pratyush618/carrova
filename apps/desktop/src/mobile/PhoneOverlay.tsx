import { useTranslation } from 'react-i18next';
import { useControllers } from '../app/controllers/Controllers';
import { usePort } from '../engine/port';
import { ForgetDialog } from '../screens/devices/ForgetDialog';
import type { Phase2Feature } from '../screens/devices/model';
import { List, Row } from '../ui';
import { PickComputerSheet } from './home/PickComputerSheet';
import { shareDefaults, type OpenedIntent } from './model';
import type { SendNote, SendTo } from './useSendNote';

/** What sits over the phone screen: a send (from Home or an opened link), or Forget. */
export type PhoneOverlayModel =
  OpenedIntent | { readonly kind: 'forget'; readonly id: string; readonly name: string };

/** A path's last part (a shared file's name). */
const fileName = (path: string): string => path.split('/').at(-1) ?? path;

/** The open overlay: pick the computer to send to, or confirm Forget. */
export function PhoneOverlay({
  overlay,
  sendTo,
  onClose,
  onForgotten,
}: {
  readonly overlay: PhoneOverlayModel;
  readonly sendTo: SendTo;
  readonly onClose: () => void;
  /** The computer is forgotten: leave its page. */
  readonly onForgotten: () => void;
}) {
  const { t } = useTranslation();
  const port = usePort();
  const { linked, devices } = useControllers();
  /** Close the sheet and run `send` to the computer picked in it. */
  const sendPicked =
    (send: (id: string) => Promise<void>, sent: SendNote['kind'] | null) => (id: string) => {
      onClose();
      const device = linked.devices.find((d) => d.id === id);
      if (device !== undefined) sendTo(device, send, sent);
    };
  /** Turn `feature` on for a computer here, as a new link starts it (files and Handoff ask). */
  const turnOn = (feature: Phase2Feature) => (id: string) => {
    linked.setFeature(id, feature, shareDefaults()[feature]);
  };
  switch (overlay.kind) {
    case 'clipboard':
      return (
        <PickComputerSheet
          title={t('mobile.home.sendClipboard')}
          feature="clipboard"
          devices={linked.devices}
          onTurnOn={turnOn('clipboard')}
          onClose={onClose}
          onSend={sendPicked(linked.sendClipboard, 'clipSent')}
        />
      );
    case 'files': {
      const { paths } = overlay;
      return (
        <PickComputerSheet
          title={t('mobile.pick.files', { count: paths.length })}
          feature="files"
          devices={linked.devices}
          onTurnOn={turnOn('files')}
          onClose={onClose}
          // The file's progress and outcome show as the transfer's own note.
          onSend={sendPicked((id) => linked.sendFiles(id, [...paths]), null)}
        >
          <List>
            {paths.map((path) => (
              <Row key={path} title={fileName(path)} />
            ))}
          </List>
        </PickComputerSheet>
      );
    }
    case 'handoff': {
      const { url } = overlay;
      return (
        <PickComputerSheet
          title={t('mobile.pick.link')}
          feature="handoff"
          devices={linked.devices}
          onTurnOn={turnOn('handoff')}
          onClose={onClose}
          onSend={sendPicked((id) => port.handoffUrl(id, url), 'linkSent')}
        >
          <p className="p-mono m-url">{url}</p>
        </PickComputerSheet>
      );
    }
    case 'text': {
      const { text } = overlay;
      return (
        <PickComputerSheet
          title={t('mobile.pick.text')}
          feature="clipboard"
          devices={linked.devices}
          onTurnOn={turnOn('clipboard')}
          onClose={onClose}
          onSend={sendPicked((id) => port.sendText(id, text), 'clipSent')}
        >
          <p className="m-text">{text}</p>
        </PickComputerSheet>
      );
    }
    case 'forget': {
      const { id, name } = overlay;
      return (
        <ForgetDialog
          name={name}
          on={{
            onCancel: onClose,
            onForget: () => {
              onClose();
              void devices.forget({ device: id }).then(() => linked.reload());
              onForgotten();
            },
          }}
        />
      );
    }
  }
}
