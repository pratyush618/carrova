import { useId, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Button, Icon, LinkButton } from '../../ui';

/**
 * The one-time error-report question, one line under the promise: off until
 * answered (either answer dismisses it), Settings keeps the switch. What a
 * report holds stays one click away, so the short question stays honest.
 */
export function ReportsBanner({ onAnswer }: { readonly onAnswer: (on: boolean) => void }) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const id = useId();
  return (
    <section className="s-home__ask" aria-labelledby={`${id}-q`}>
      <Icon name="info" size="sm" />
      <h2 id={`${id}-q`} className="s-home__ask-q">
        {t('start.reports.title')}
      </h2>
      <LinkButton
        aria-expanded={open}
        aria-controls={`${id}-body`}
        onClick={() => {
          setOpen((o) => !o);
        }}
      >
        {t('start.reports.whatsSent')}
      </LinkButton>
      <Button
        size="sm"
        onClick={() => {
          onAnswer(true);
        }}
      >
        {t('start.reports.allow')}
      </Button>
      <Button
        kind="quiet"
        size="sm"
        onClick={() => {
          onAnswer(false);
        }}
      >
        {t('start.reports.deny')}
      </Button>
      <p id={`${id}-body`} className="s-home__ask-body" hidden={!open}>
        {t('start.reports.body')}
      </p>
    </section>
  );
}
