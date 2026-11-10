import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { send } from '../../i18n/en/send';
import { AppGlyph, Button, Check, GroupHead, List, Row } from '../../ui';
import type { FoundGroup, FoundIntents, SecretItem } from './model';

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

function AppGroup({ group, on }: { readonly group: FoundGroup; readonly on: SelectionIntents }) {
  const { t } = useTranslation();
  const format = useFormat();
  const name = categoryName(group.category, t);
  const picked = group.apps.filter((a) => a.included).length;
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
        title={name}
        aside={t('summary.ofTotal', { done: picked, total: group.apps.length })}
      />
      <List label={name}>
        {group.apps.map((app) => (
          <Row
            key={app.id}
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
        ))}
      </List>
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
        <AppGroup key={group.category} group={group} on={on} />
      ))}
    </>
  );
}
