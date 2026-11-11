import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  AppGlyph,
  Badge,
  Button,
  GroupHead,
  Head,
  HubLayout,
  Icon,
  Kbd,
  LinkButton,
  List,
  Row,
} from '../../ui';
import type { HistoryEntry } from '../history/model';
import { KIND_ICON, moveSub, moveTag, moveTitle } from '../history/moveWords';
import type { HomeIntents, HomeModel } from './model';
import { FileManagerBanner } from './FileManagerBanner';
import { ReportsBanner } from './ReportsBanner';
import { VerbTile } from './VerbTile';
import './start.css';

export interface HomeScreenProps {
  readonly model: HomeModel;
  readonly on: HomeIntents;
}

/** Where single-key shortcuts must not fire: typing, or anything over Home. */
const NO_SHORTCUTS =
  'input, textarea, select, [contenteditable], [role=dialog], [role=alertdialog]';

/** S, R and O start the matching verb while nothing else has the keyboard. */
function useShortcuts(on: HomeIntents): void {
  useEffect(() => {
    const keys: Partial<Record<string, () => void>> = {
      s: on.onSend,
      r: on.onReceive,
      o: on.onUseFile,
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.repeat || event.ctrlKey || event.metaKey || event.altKey)
        return;
      if (event.target instanceof Element && event.target.closest(NO_SHORTCUTS)) return;
      const act = keys[event.key.toLowerCase()];
      if (act === undefined) return;
      event.preventDefault();
      act();
    };
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('keydown', onKey);
    };
  }, [on]);
}

function RecentRow({
  entry,
  now,
  onUndo,
}: {
  readonly entry: HistoryEntry;
  readonly now: Date;
  readonly onUndo: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const title = moveTitle(entry, t);
  const tag = moveTag(entry, t);
  return (
    <Row
      lead={
        <AppGlyph
          glyph={{ icon: KIND_ICON[entry.kind] }}
          tone={tag?.tone === 'success' ? 'success' : 'neutral'}
        />
      }
      title={title}
      sub={moveSub(entry, now, t, format)}
      trail={
        <>
          {tag && (
            <Badge tone={tag.tone} {...(tag.icon && { icon: tag.icon })}>
              {tag.label}
            </Badge>
          )}
          {entry.undoable && (
            <Button
              kind="quiet"
              size="sm"
              icon="undo"
              aria-label={t('start.home.undoMove', { move: title })}
              onClick={onUndo}
            >
              {t('start.home.undo')}
            </Button>
          )}
        </>
      }
    />
  );
}

/** The three steps of a move, named once for first-time users. */
function HowItWorks() {
  const { t } = useTranslation();
  const steps = ['pick', 'pair', 'review'] as const;
  return (
    <ol className="s-home__how" aria-label={t('start.home.how.label')}>
      {steps.map((step, i) => (
        <li key={step}>
          <b>
            <i aria-hidden="true">{i + 1}</i>
            {t(`start.home.how.${step}.title`)}
          </b>
          {t(`start.home.how.${step}.body`)}
        </li>
      ))}
    </ol>
  );
}

/**
 * Two verbs, nothing else competes. First run teaches by doing: a hint on the
 * likely first step, the three steps and the promise. Returning users get
 * their recent moves with Undo instead.
 */
export function HomeScreen({ model, on }: HomeScreenProps) {
  const { t } = useTranslation();
  const [hint, setHint] = useState(true);
  useShortcuts(on);
  const first = model.recent.length === 0;
  const mode = first ? 'first' : 'returning';
  return (
    <HubLayout variant="center">
      <Head hero title={t(`start.home.${mode}.title`)} lede={t(`start.home.${mode}.lede`)} />
      <div className="s-home__verbs">
        <VerbTile
          verb="send"
          shortcut="S"
          {...(first &&
            hint && {
              hint: {
                label: t('start.home.hint'),
                onDismiss: () => {
                  setHint(false);
                },
              },
            })}
          onClick={on.onSend}
        />
        <VerbTile verb="receive" shortcut="R" onClick={on.onReceive} />
      </div>
      <p className="s-home__file">
        <LinkButton aria-keyshortcuts="O" onClick={on.onUseFile}>
          {t('start.home.useFile')}
        </LinkButton>
        <span>{t('start.home.useFileSub')}</span>
        <span className="s-home__file-key" aria-hidden="true">
          <Kbd>O</Kbd>
        </span>
      </p>
      {first ? (
        <HowItWorks />
      ) : (
        <section aria-labelledby="s-home-recent">
          <GroupHead
            title={<span id="s-home-recent">{t('start.home.recent')}</span>}
            aside={<LinkButton onClick={on.onHistory}>{t('start.home.seeAll')}</LinkButton>}
          />
          <List>
            {model.recent.map((entry) => (
              <RecentRow
                key={entry.id}
                entry={entry}
                now={model.now}
                onUndo={() => {
                  on.onUndo(entry.id);
                }}
              />
            ))}
          </List>
        </section>
      )}
      {/* The questions tuck under the promise, one at a time: Home must fit 1100x720 whole. */}
      {(first || model.askReports || model.askFileManager) && (
        <div className="s-home__foot">
          {first && (
            <p className="s-home__promise">
              <Icon name="shield" size="sm" />
              {t('start.home.promise')}
            </p>
          )}
          {model.askReports && <ReportsBanner onAnswer={on.onErrorReports} />}
          {model.askFileManager && (
            <FileManagerBanner failed={model.fileManagerFailed} onAnswer={on.onFileManager} />
          )}
        </div>
      )}
    </HubLayout>
  );
}
