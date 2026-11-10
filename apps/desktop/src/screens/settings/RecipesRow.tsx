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
    case 'updated':
      return t('settings.recipes.updated', { version: check.version });
    case 'failed':
      return errorText(check.error, t).body;
  }
}

/** Which recipes are in use, and "Check now" with its outcome in words. */
export function RecipesRow({
  recipes,
  onCheck,
}: {
  readonly recipes: Recipes;
  readonly onCheck: () => void;
}) {
  const { t } = useTranslation();
  const outcome = useCheckText(recipes.check);
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
        <Button
          size="sm"
          busy={recipes.check.status === 'checking'}
          disabled={recipes.check.status === 'checking'}
          onClick={onCheck}
        >
          {t('settings.recipes.check')}
        </Button>
      }
    />
  );
}
