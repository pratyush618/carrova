import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { Button, Muted, Segmented, Sheet } from '../../ui';

/**
 * Send something to one computer: the computers it can reach now, the
 * first one picked; `children` shows what goes.
 */
export function PickComputerSheet({
  title,
  devices,
  onSend,
  onClose,
  children,
}: {
  readonly title: string;
  /** Linked computers the feature reaches now. */
  readonly devices: readonly LinkedDeviceView[];
  readonly onSend: (id: string) => void;
  readonly onClose: () => void;
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const [picked, setPicked] = useState<string | null>(null);
  // The first one until the user picks (the list may load after the sheet opens).
  const id = picked ?? devices[0]?.id ?? null;
  return (
    <Sheet
      title={title}
      onClose={onClose}
      foot={
        <>
          <Button onClick={onClose}>{t('mobile.pick.cancel')}</Button>
          <Button
            kind="primary"
            disabled={id === null}
            onClick={() => {
              if (id !== null) onSend(id);
            }}
          >
            {t('mobile.pick.send')}
          </Button>
        </>
      }
    >
      {children}
      {devices.length === 0 ? (
        <Muted>{t('mobile.home.noneNearby')}</Muted>
      ) : (
        <Segmented
          label={t('mobile.pick.to')}
          options={devices.map((d) => ({ value: d.id, label: d.name }))}
          value={id}
          onChange={setPicked}
        />
      )}
    </Sheet>
  );
}
