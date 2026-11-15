import type { AskKind } from '../../engine/types';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useFormat, type Formatters } from '../../i18n/useFormat';
import { Button, Toast } from '../../ui';

/** The question, and its two answers, for each kind of offer. */
function words(kind: AskKind, name: string, t: TFunction, format: Formatters) {
  if (kind.kind === 'file')
    return {
      text: t('ecosystem.ask.file', { name, file: kind.name, size: format.bytes(kind.size) }),
      yes: t('ecosystem.ask.accept'),
      no: t('ecosystem.ask.decline'),
    };
  return {
    text:
      kind.file === null
        ? t('ecosystem.ask.url', { name, host: kind.url_host ?? t('ecosystem.ask.someLink') })
        : t('ecosystem.ask.handoffFile', { name, file: kind.file }),
    yes: t('ecosystem.ask.open'),
    no: t('ecosystem.ask.notNow'),
  };
}

/** A device offers a file or something to continue here: asked every time, on this side. */
export function AskToast({
  device,
  kind,
  onAnswer,
}: {
  /** The offering device's name. */
  readonly device: string;
  readonly kind: AskKind;
  readonly onAnswer: (accept: boolean) => void;
}) {
  const { t } = useTranslation();
  const { text, yes, no } = words(kind, device, t, useFormat());
  return (
    <Toast
      icon={kind.kind === 'file' ? 'file' : 'external'}
      dismissLabel={no}
      onDismiss={() => {
        onAnswer(false);
      }}
      action={
        <Button
          kind="primary"
          size="sm"
          onClick={() => {
            onAnswer(true);
          }}
        >
          {yes}
        </Button>
      }
    >
      {text}
    </Toast>
  );
}
