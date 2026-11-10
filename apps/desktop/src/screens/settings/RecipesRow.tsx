import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import { AppGlyph, Button, Row } from '../../ui';
import type { RecipeCheck, SettingsModel } from './model';

type Recipes = NonNullable<SettingsModel['recipes']>;

function useCheckText(check: RecipeCheck): string | null {
  const { t } = useTranslation();
  switch (check.status) {
    case 'idle':
      return null;
    case 'checking':
      return t('settings.recipes.checking');
    case 'up_to_date':
      return t('settings.recipes.upToDate');
    case 'retry_later':
      return t('settings.recipes.retryLater');
    case 'available':
      return t('settings.recipes.available', { version: check.version });
    case 'refused':
      return t('settings.recipes.refused', { version: check.version });
    case 'installing':
      return t('settings.recipes.installing', { version: check.version });
    case 'updated':
      return t('settings.recipes.updated', { version: check.version });
    case 'failed':
      return errorText(check.error, t).body;
  }
}

/** Which recipes are in use, "Check now" with its outcome in words, and "Install update" once offered. */
export function RecipesRow({
  recipes,
  onCheck,
  onInstall,
}: {
  readonly recipes: Recipes;
  readonly onCheck: () => void;
  readonly onInstall: () => void;
}) {
  const { t } = useTranslation();
  const outcome = useCheckText(recipes.check);
  const installing = recipes.check.status === 'installing';
  const offer = installing || recipes.check.status === 'available';
  const counts = {
    verified: recipes.verified,
    community: recipes.community,
    unverified: recipes.unverified,
  };
  const source =
    recipes.source.kind === 'dev'
      ? t('settings.recipes.dev', counts)
      : t(`settings.recipes.${recipes.source.kind}`, {
          ...counts,
          version: recipes.source.version,
        });
  return (
    <Row
      lead={
        <AppGlyph
          glyph={{ icon: 'box' }}
          tone={recipes.source.kind === 'dev' ? 'warning' : 'success'}
        />
      }
      title={t('settings.recipes.title')}
      sub={
        <>
          {source}
          {outcome !== null && (
            <>
              {' '}
              <span role="status">{outcome}</span>
            </>
          )}
        </>
      }
      trail={
        offer ? (
          <Button
            kind="primary"
            size="sm"
            busy={installing}
            disabled={installing}
            onClick={onInstall}
          >
            {t('settings.recipes.install')}
          </Button>
        ) : (
          <Button
            size="sm"
            busy={recipes.check.status === 'checking'}
            disabled={recipes.check.status === 'checking'}
            onClick={onCheck}
          >
            {t('settings.recipes.check')}
          </Button>
        )
      }
    />
  );
}
