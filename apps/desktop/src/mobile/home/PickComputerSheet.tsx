import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { Button, Segmented, Sheet } from '../../ui';

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
  /** Linked computers the feature reaches now (at least one). */
  readonly devices: readonly LinkedDeviceView[];
  readonly onSend: (id: string) => void;
  readonly onClose: () => void;
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const [id, setId] = useState(devices[0]?.id ?? null);
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
      <Segmented
        label={t('mobile.pick.to')}
        options={devices.map((d) => ({ value: d.id, label: d.name }))}
        value={id}
        onChange={setId}
      />
    </Sheet>
  );
}
