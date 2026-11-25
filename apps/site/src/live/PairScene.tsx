import { useEffect, useState } from 'react';
import { revealDuration } from '../../../desktop/src/ui/CodeDisplay/flaps';
import { SendCodeScreen } from '../../../desktop/src/screens/send/SendCodeScreen';
import { SendLinkedScreen } from '../../../desktop/src/screens/send/SendLinkedScreen';
import * as fx from '../../../desktop/src/screens/send/fixtures';
import { AppWindow, flowAt } from './AppWindow';

const ignore = (): void => undefined;
// Long enough to read the code once it has settled, before the other computer types it.
const READ_MS = 1600;

/**
 * The send side of pairing, played like the app plays it: the code flips in,
 * then the other computer types it and the two link. Each "play" starts over.
 */
export function PairScene({ plays }: { readonly plays: number }) {
  return <PairRun key={plays} />;
}

function PairRun() {
  const [linked, setLinked] = useState(false);

  useEffect(() => {
    const code = 'code' in fx.pairingWaiting ? fx.pairingWaiting.code : [];
    const timer = window.setTimeout(
      () => {
        setLinked(true);
      },
      revealDuration(code.join('').length) + READ_MS,
    );
    return () => {
      window.clearTimeout(timer);
    };
  }, []);

  return linked ? (
    <AppWindow view={flowAt('send', 'live')}>
      <SendLinkedScreen model={fx.linkedSameOs} on={{ onCancel: ignore }} />
    </AppWindow>
  ) : (
    <AppWindow view={flowAt('send', 'code')}>
      <SendCodeScreen
        model={fx.pairingWaiting}
        move={fx.move}
        on={{ onBack: ignore, onNewCode: ignore, onHome: ignore }}
      />
    </AppWindow>
  );
}
