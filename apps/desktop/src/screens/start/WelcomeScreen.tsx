import { useTranslation } from 'react-i18next';
import {
  BareLayout,
  Button,
  Head,
  Icon,
  List,
  Row,
  StandaloneRow,
  Toggle,
  Wordmark,
} from '../../ui';
import type { WelcomeIntents, WelcomeModel } from './model';

export interface WelcomeScreenProps {
  readonly model: WelcomeModel;
  readonly on: WelcomeIntents;
}

/** First run: what moves, the three promises, and crash-report consent (off, asked once). */
export function WelcomeScreen({ model, on }: WelcomeScreenProps) {
  const { t } = useTranslation();
  return (
    <BareLayout>
      <Wordmark />
      <Head hero title={t('start.welcome.title')} lede={t('start.welcome.lede')} />
      <List kind="plain" label={t('start.welcome.promises')}>
        <Row
          lead={<Icon name="lock" />}
          title={t('start.welcome.direct.title')}
          sub={t('start.welcome.direct.sub')}
        />
        <Row
          lead={<Icon name="check" />}
          title={t('start.welcome.approve.title')}
          sub={t('start.welcome.approve.sub')}
        />
        <Row
          lead={<Icon name="undo" />}
          title={t('start.welcome.undo.title')}
          sub={t('start.welcome.undo.sub')}
        />
      </List>
      <StandaloneRow
        title={t('start.welcome.reports.title')}
        sub={t('start.welcome.reports.sub')}
        trail={
          <Toggle
            on={model.errorReports}
            label={t('start.welcome.reports.title')}
            onChange={on.onErrorReports}
          />
        }
      />
      <div>
        <Button kind="primary" onClick={on.onStart}>
          {t('start.welcome.start')}
        </Button>
      </div>
    </BareLayout>
  );
}
