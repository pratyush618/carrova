import type { ReactNode } from 'react';
import { NewCodeDialog } from '../../../screens/send/NewCodeDialog';
import { useControllers } from '../../controllers/Controllers';
import type { View } from '../../nav/navMachine';
import { useSession } from '../../state/EngineState';

/**
 * Dialogs the engine opens rather than the user: "a wrong code was tried,
 * show a new one?" while the sender shows its code.
 */
export function useAutoOverlays(view: View): ReactNode[] {
  const { send } = useControllers();
  const progress = useSession(send.state.session);
  const prompt = progress.newCodePrompt;
  const pairing = view.kind === 'flow' && view.at.flow === 'send' && view.at.step === 'code';
  if (!pairing || prompt === null) return [];
  return [
    <NewCodeDialog
      key="new-code"
      model={prompt}
      on={{
        onStop: () => {
          send.answerPrompt(false);
        },
        onNewCode: () => {
          send.answerPrompt(true);
        },
      }}
    />,
  ];
}
