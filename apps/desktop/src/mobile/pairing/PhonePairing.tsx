import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { Button, Empty, Field, Input, Note } from '../../ui';
import { PhonePage } from '../PhonePage';
import { usePhonePairing } from './usePhonePairing';

/**
 * Add a computer: not on Wi-Fi, scan or type the code, linking (this
 * phone's key shows for the computer's check), then linked or failed.
 */
export function PhonePairing({
  onClose,
  onChoose,
}: {
  readonly onClose: () => void;
  /** Linked: choose what to share with `device` (`null` when the list doesn't name it). */
  readonly onChoose: (device: LinkedDeviceView | null) => void;
}) {
  const { t } = useTranslation();
  const { step, ownFingerprint, scan, type, link, retry } = usePhonePairing();
  const [code, setCode] = useState('');
  const wide = (label: string, onClick: () => void) => (
    <div className="m-wide">
      <Button kind="primary" size="lg" onClick={onClick}>
        {label}
      </Button>
    </div>
  );
  switch (step.kind) {
    case 'checking':
      return <PhonePage onBack={onClose}>{null}</PhonePage>;
    case 'offline':
      return (
        <PhonePage onBack={onClose} center>
          <Empty
            art="wifi"
            title={t('mobile.pair.offline')}
            body={t('mobile.pair.offlineBody')}
            action={wide(t('mobile.pair.retry'), retry)}
          />
        </PhonePage>
      );
    case 'add':
      return (
        <PhonePage title={t('mobile.add.title')} onBack={onClose}>
          <p>{t('mobile.add.lede')}</p>
          <div className="m-wide">
            <Button kind="primary" size="lg" icon="laptop" onClick={scan}>
              {t('mobile.add.scan')}
            </Button>
            <Button onClick={type}>{t('mobile.add.type')}</Button>
          </div>
        </PhonePage>
      );
    case 'typing': {
      const typed = code.trim() !== '';
      return (
        <PhonePage title={t('mobile.type.title')} onBack={retry}>
          <p>{t('mobile.type.lede')}</p>
          <Field label={t('mobile.type.field')}>
            {(control) => (
              <Input
                {...control}
                mono
                autoComplete="off"
                autoCapitalize="none"
                spellCheck={false}
                value={code}
                onChange={(event) => {
                  setCode(event.target.value);
                }}
                onKeyDown={(event) => {
                  if (event.key === 'Enter' && typed) link(code);
                }}
              />
            )}
          </Field>
          <Button
            kind="primary"
            size="lg"
            disabled={!typed}
            onClick={() => {
              link(code);
            }}
          >
            {t('mobile.type.link')}
          </Button>
        </PhonePage>
      );
    }
    case 'linking':
      return (
        <PhonePage center>
          <Empty
            art="lock"
            title={t('mobile.pair.linking')}
            body={t('mobile.pair.linkingBody')}
            action={
              <Button kind="quiet" onClick={onClose}>
                {t('mobile.pair.cancel')}
              </Button>
            }
          />
          {ownFingerprint !== null && (
            <>
              <Note center>
                <span className="p-mono">
                  {t('mobile.pair.thisPhone', { fingerprint: ownFingerprint })}
                </span>
              </Note>
              <Note center>{t('mobile.pair.confirmThere')}</Note>
            </>
          )}
        </PhonePage>
      );
    case 'linked':
      return (
        <PhonePage center>
          <Empty
            art="check"
            title={t('mobile.pair.linked')}
            body={
              step.device === null
                ? t('mobile.pair.linkedUnnamed')
                : t('mobile.pair.linkedBody', { name: step.device.name })
            }
            action={wide(t('mobile.pair.choose'), () => {
              onChoose(step.device);
            })}
          />
        </PhonePage>
      );
    case 'failed':
      return (
        <PhonePage onBack={onClose} center>
          <Empty
            art="warn"
            tone="danger"
            title={t('mobile.pair.failed')}
            body={t('mobile.pair.failedBody')}
            action={wide(t('mobile.pair.retry'), retry)}
          />
        </PhonePage>
      );
  }
}
