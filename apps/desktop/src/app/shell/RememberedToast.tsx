import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Toast } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import { useRemembered, type Remembered } from '../state/EngineState';

/**
 * The device a move just remembered, until the user closes the notice. Held
 * by the shell: the window remounts on every navigation, the notice must not.
 */
export function useRememberedNotice(): {
  readonly remembered: Remembered | null;
  readonly close: () => void;
} {
  const remembered = useRemembered();
  const [closed, setClosed] = useState<Remembered | null>(null);
  return {
    remembered: remembered === closed ? null : remembered,
    close: () => {
      setClosed(remembered);
    },
  };
}

/** "Remembered Ana's laptop (7F3A·19C2)" after a move, with Forget right there. */
export function RememberedToast({
  remembered,
  onClose,
}: {
  readonly remembered: Remembered;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  const { devices } = useControllers();
  const [forgetting, setForgetting] = useState(false);
  return (
    <Toast
      onDismiss={onClose}
      action={
        <Button
          kind="quiet"
          size="sm"
          disabled={forgetting}
          onClick={() => {
            setForgetting(true);
            // A refused Forget leaves the notice up, to try again.
            void devices.forget({ device: remembered.fingerprint }).then((done) => {
              if (done) onClose();
              else setForgetting(false);
            });
          }}
        >
          {t('nearby.actions.forget')}
        </Button>
      }
    >
      {t('nearby.remembered', { name: remembered.name, fingerprint: remembered.fingerprint })}
    </Toast>
  );
}
