import { SendLinkedScreen } from '../../../desktop/src/screens/send/SendLinkedScreen';
import * as fx from '../../../desktop/src/screens/send/fixtures';
import { AppWindow, flowAt } from './AppWindow';

const ignore = (): void => undefined;

/** Two computers, linked: the handshake plays again on every "play", then traffic flows. */
export function LinkedScene({ plays }: { readonly plays: number }) {
  return (
    <div className="live-linked">
      <AppWindow view={flowAt('send', 'live')}>
        <SendLinkedScreen key={plays} model={fx.linkedSameOs} on={{ onCancel: ignore }} />
      </AppWindow>
    </div>
  );
}
