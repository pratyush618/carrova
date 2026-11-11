import { useTranslation } from 'react-i18next';
import { AppGlyph, Badge, Icon, List, Muted, Row } from '../../ui';
import type { CatalogDetail, RecipeItem } from './model';
import { TierBadge } from './TierBadge';

function Items({
  label,
  items,
}: {
  readonly label: string;
  readonly items: readonly RecipeItem[];
}) {
  const { t } = useTranslation();
  if (items.length === 0) return null;
  return (
    <section>
      <h3 className="s-detail__h">{label}</h3>
      <List kind="plain" label={label}>
        {items.map((item) => (
          <Row
            key={item.id}
            title={item.label}
            {...(item.tag === 'sensitive' && { sub: t('apps.startsOff') })}
            off={item.tag === null || item.tag === 'unmovable'}
            trail={
              item.tag === 'sensitive' ? (
                <Badge tone="warning">{t('badge.sensitive')}</Badge>
              ) : item.tag === 'unmovable' ? (
                <Badge>{t('badge.cantMove')}</Badge>
              ) : undefined
            }
          />
        ))}
      </List>
    </section>
  );
}

/** What Carrova moves for the selected app, what it leaves out, and who checked the recipe. */
export function AppDetail({ app }: { readonly app: CatalogDetail | null }) {
  const { t } = useTranslation();
  if (app === null) {
    return (
      <div className="s-detail s-detail--empty">
        <AppGlyph glyph={{ icon: 'apps' }} />
        <b>{t('apps.pick')}</b>
        <Muted>{t('apps.pickBody')}</Muted>
      </div>
    );
  }
  return (
    <div className="s-detail">
      <div className="s-detail__head">
        <AppGlyph glyph={app.glyph} />
        <div>
          <h2>{app.name}</h2>
          <Muted>{app.here ? t('apps.found') : t('apps.notFound')}</Muted>
        </div>
      </div>
      <p className="s-detail__trust">
        <Icon name="shield" size="sm" />
        <TierBadge tier={app.tier} />
        {t(`apps.tiers.${app.tier}`)}
      </p>
      <Items label={t('apps.moves')} items={app.moves} />
      <Items label={t('apps.leftOut')} items={app.leftOut} />
    </div>
  );
}
