import { useTranslation } from 'react-i18next';
import { Badge, Button, List, Muted, Row, Sheet } from '../../ui';
import type { RecipeIntents, RecipeItem, RecipeModel } from './model';
import { TierBadge } from './TierBadge';

function ItemBadge({ tag }: { readonly tag: RecipeItem['tag'] }) {
  const { t } = useTranslation();
  switch (tag) {
    case 'suggested':
      return <Badge>{t('badge.suggested')}</Badge>;
    case 'sensitive':
      return <Badge tone="warning">{t('badge.sensitive')}</Badge>;
    case 'unmovable':
      return <Badge>{t('badge.cantMove')}</Badge>;
    case null:
      return null;
  }
}

/** What Carrova can move for one app, and how much the recipe is trusted. */
export function RecipeSheet({
  model,
  on,
}: {
  readonly model: RecipeModel;
  readonly on: RecipeIntents;
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={model.name}
      onClose={on.onClose}
      foot={<Button onClick={on.onClose}>{t('ui.close')}</Button>}
    >
      <Muted>{model.description}</Muted>
      <Muted>{t('apps.recipe.intro', { app: model.name })}</Muted>
      <List label={t('apps.recipe.items')}>
        {model.items.map((item) => (
          <Row key={item.id} title={item.label} trail={<ItemBadge tag={item.tag} />} />
        ))}
      </List>
      <List kind="plain">
        <Row
          title={t('apps.recipe.tier')}
          sub={t(`apps.recipe.tiers.${model.tier}`)}
          trail={<TierBadge tier={model.tier} />}
        />
        <Row
          title={t('apps.recipe.here')}
          sub={model.installed ? t('apps.installed') : t('apps.notInstalled')}
        />
      </List>
    </Sheet>
  );
}
