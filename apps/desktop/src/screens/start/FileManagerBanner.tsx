import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Icon, LinkButton } from '../../ui';

/**
 * The one-time "Send with Carrova" question, the same one line as the
 * error-report question: nothing is written until the user says yes, either
 * answer that went through dismisses it, and Settings keeps the switch.
 * `failed`: adding the entry failed; the question stays so "Add it" can retry.
 */
export function FileManagerBanner({
  failed,
  onAnswer,
}: {
  readonly failed: boolean;
  readonly onAnswer: (yes: boolean) => void;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <section className="s-home__ask" aria-labelledby={`${id}-q`}>
      <Icon name="send" size="sm" />
      <h2 id={`${id}-q`} className="s-home__ask-q">
        {t('start.fileManager.title')}
      </h2>
      <LinkButton
        aria-expanded={open}
        aria-controls={`${id}-body`}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        {t('start.fileManager.how')}
      </LinkButton>
      <Button
        size="sm"
        onClick={() => {
          onAnswer(true);
        }}
      >
        {t('start.fileManager.yes')}
      </Button>
      <Button
        kind="quiet"
        size="sm"
        onClick={() => {
          onAnswer(false);
        }}
      >
        {t('start.fileManager.no')}
      </Button>
      <p id={`${id}-body`} className="s-home__ask-body" hidden={!open}>
        {t('start.fileManager.body')}
      </p>
      {failed && (
        <p role="alert" className="s-home__ask-body">
          {t('start.fileManager.failed')}
        </p>
      )}
    </section>
  );
}
