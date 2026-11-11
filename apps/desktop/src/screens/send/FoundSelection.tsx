import type { TFunction } from 'i18next';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { send } from '../../i18n/en/send';
import {
  AppGlyph,
  Button,
  Check,
  GroupHead,
  Icon,
  LinkButton,
  List,
  Row,
  VirtualList,
} from '../../ui';
import type { FoundApp, FoundGroup, FoundIntents, SecretItem } from './model';

type SelectionIntents = Pick<FoundIntents, 'onToggleApp' | 'onConfirmKey' | 'onLeaveKey'>;

type KnownCategory = keyof typeof send.found.categories;
const known = (id: string): id is KnownCategory => Object.hasOwn(send.found.categories, id);

/** A recipe category's name. shortcut: unknown categories show their id, until recipes carry names. */
export const categoryName = (id: string, t: TFunction): string =>
  known(id) ? t(`send.found.categories.${id}`) : id;

function SecretsGroup({
  secrets,
  on,
}: {
  readonly secrets: readonly SecretItem[];
  readonly on: SelectionIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const brought = secrets.filter((s) => s.brought).length;
  const next = secrets.find((s) => !s.brought);
  return (
    <section>
      <GroupHead
        lead={
          <Check
            warn
            value={next === undefined ? true : brought > 0 ? 'mixed' : false}
            label={t('send.found.bringKeys')}
            onChange={() => {
              // Never bulk-confirms: asks for the next key, or leaves them all (the safe way).
              if (next) on.onConfirmKey(next.key);
              else for (const s of secrets) on.onLeaveKey(s.key);
            }}
          />
        }
        title={t('send.found.secrets')}
        aside={t('send.found.secretsCount', { brought, total: secrets.length })}
      />
      <List label={t('send.found.secrets')} warn={t('send.found.secretsBand')}>
        {secrets.map((secret) => (
          <Row
            key={secret.key}
            off={!secret.brought}
            lead={
              <>
                <Check
                  warn
                  value={secret.brought}
                  label={t('send.found.bring', { app: secret.label })}
                  onChange={(bring) => {
                    if (bring) on.onConfirmKey(secret.key);
                    else on.onLeaveKey(secret.key);
                  }}
                />
                <AppGlyph glyph={secret.app.glyph} tone="warning" />
              </>
            }
            title={secret.label}
            sub={secret.app.name}
            trail={
              <>
                {!secret.brought && (
                  <Button
                    size="sm"
                    aria-label={t('send.found.includeKey', { item: secret.label })}
                    onClick={() => {
                      on.onConfirmKey(secret.key);
                    }}
                  >
                    {t('send.found.include')}
                  </Button>
                )}
                <span>{format.bytes(secret.bytes)}</span>
              </>
            }
          />
        ))}
      </List>
    </section>
  );
}

/** Rows an opened group shows before "Show N more". */
const FIRST_ROWS = 8;
/** Past this many rows a list mounts only what is in view. */
const VIRTUALISE_ABOVE = 200;
/** An app row's height, px: virtualised rows must all be this tall. */
const APP_ROW = 56;

function AppRow({ app, on }: { readonly app: FoundApp; readonly on: SelectionIntents }) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Row
      off={!app.included}
      lead={
        <>
          <Check
            value={app.included}
            label={t('send.found.bring', { app: app.name })}
            onChange={(included) => {
              on.onToggleApp(app.id, included);
            }}
          />
          <AppGlyph glyph={app.glyph} />
        </>
      }
      title={app.name}
      sub={app.what}
      trail={format.bytes(app.bytes)}
    />
  );
}

function AppGroup({ group, on }: { readonly group: FoundGroup; readonly on: SelectionIntents }) {
  const { t } = useTranslation();
  const name = categoryName(group.category, t);
  const [toggled, setToggled] = useState<boolean | null>(null);
  const [showAll, setShowAll] = useState(false);
  const open = toggled ?? !group.collapsed;
  const picked = group.apps.filter((a) => a.included).length;
  const rows = showAll ? group.apps : group.apps.slice(0, FIRST_ROWS);
  const more = group.apps.length - rows.length;
  return (
    <section>
      <GroupHead
        lead={
          <Check
            value={picked === group.apps.length ? true : picked > 0 ? 'mixed' : false}
            label={t('send.found.bringAll', { group: name })}
            onChange={(included) => {
              for (const app of group.apps) on.onToggleApp(app.id, included);
            }}
          />
        }
        title={
          <button
            type="button"
            className="s-found-toggle"
            aria-expanded={open}
            onClick={() => {
              setToggled(!open);
            }}
          >
            <Icon name="chev" size="sm" />
            {name}
          </button>
        }
        aside={t('summary.ofTotal', { done: picked, total: group.apps.length })}
      />
      {open &&
        (rows.length > VIRTUALISE_ABOVE ? (
          <VirtualList
            label={name}
            items={rows}
            rowHeight={APP_ROW}
            render={(app) => <AppRow app={app} on={on} />}
          />
        ) : (
          <List label={name}>
            {rows.map((app) => (
              <AppRow key={app.id} app={app} on={on} />
            ))}
          </List>
        ))}
      {open && more > 0 && (
        <LinkButton
          onClick={() => {
            setShowAll(true);
          }}
        >
          {t('send.found.showMore', { count: more, group: name })}
        </LinkButton>
      )}
    </section>
  );
}

/** Keys and secrets first (each starts off, brought one by one), then apps by category, ticked as suggested. */
export function FoundSelection({
  groups,
  secrets,
  on,
}: {
  readonly groups: readonly FoundGroup[];
  readonly secrets: readonly SecretItem[];
  readonly on: SelectionIntents;
}) {
  return (
    <>
      {secrets.length > 0 && <SecretsGroup secrets={secrets} on={on} />}
      {groups.map((group) => (
        // Narrowing a search or view flips `collapsed`: remount so it re-opens the group.
        <AppGroup key={`${group.category}:${String(group.collapsed)}`} group={group} on={on} />
      ))}
    </>
  );
}
