import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { AppGlyph, Badge, Decision, GroupHead, Head, List, Row, Toggle } from '../../ui';
import { askOptions } from '../shared/askOptions';
import type { FoundApp, FoundIntents, SensitiveAsk } from './model';

export interface FoundSelectionProps {
  readonly apps: readonly FoundApp[];
  readonly asks: readonly SensitiveAsk[];
  readonly on: Pick<FoundIntents, 'onToggleApp' | 'onAnswer'>;
}

/** Totals over the apps the user is bringing. */
export function selectionTotals(apps: readonly FoundApp[]): { files: number; bytes: number } {
  return apps
    .filter((a) => a.included)
    .reduce((sum, a) => ({ files: sum.files + a.files, bytes: sum.bytes + a.bytes }), {
      files: 0,
      bytes: 0,
    });
}

/** Apps as rows with sizes; risky items asked once, in plain words, starting on "Leave behind". */
export function FoundSelection({ apps, asks, on }: FoundSelectionProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const totals = selectionTotals(apps);
  const options = askOptions(t);
  return (
    <>
      <Head
        title={t('send.found.title')}
        lede={t('send.found.lede', {
          apps: t('count.apps', { count: apps.length }),
          files: t('count.files', { count: totals.files }),
          size: format.bytes(totals.bytes),
        })}
      />
      <List label={t('send.found.appsLabel')}>
        {apps.map((app) => (
          <Row
            key={app.id}
            lead={<AppGlyph glyph={app.glyph} />}
            title={app.name}
            sub={app.what}
            off={!app.included}
            trail={
              <>
                <span>{format.bytes(app.bytes)}</span>
                <Toggle
                  on={app.included}
                  label={t('send.found.bring', { app: app.name })}
                  onChange={(included) => {
                    on.onToggleApp(app.id, included);
                  }}
                />
              </>
            }
          />
        ))}
      </List>
      {asks.length > 0 && (
        <div>
          <GroupHead
            title={t('send.found.needsOk')}
            aside={<Badge tone="warning">{t('send.found.offUnless')}</Badge>}
          />
          {asks.map((ask) => (
            <Decision
              key={ask.key}
              icon={ask.kind === 'secret' ? 'key' : 'warn'}
              title={ask.title}
              why={ask.why ?? t('send.found.secretWhy')}
              options={options}
              value={ask.answer}
              onChange={(answer) => {
                on.onAnswer(ask.key, answer);
              }}
            />
          ))}
        </div>
      )}
    </>
  );
}
