import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  Badge,
  Check,
  LinkButton,
  LockedCheck,
  Path,
  Tree,
  TreeRow,
  type CheckValue,
} from '../../ui';
import type { CustomizeIntents, ItemDetail, TreeApp, TreeItem, TreePath } from './model';

type TreeIntents = Pick<
  CustomizeIntents,
  'onToggleApp' | 'onToggleItem' | 'onTogglePath' | 'onExpand' | 'onIncludeAnyway'
>;

/** An app's box: ticked, unticked, or mixed when only some items move. */
export function appCheck(app: TreeApp): CheckValue {
  const movable = app.items.filter((i) => i.tag?.kind !== 'unmovable');
  const picked = movable.filter((i) => i.picked).length;
  if (picked === 0) return false;
  return picked === movable.length ? true : 'mixed';
}

function Detail({ detail }: { readonly detail: ItemDetail }) {
  const { t } = useTranslation();
  switch (detail.kind) {
    case 'path':
      return (
        <small>
          <Path>{detail.path}</Path>
        </small>
      );
    case 'files':
      return <small>{t('send.customize.files', { count: detail.count })}</small>;
    case 'extensions':
      return <small>{t('send.customize.extensions', { count: detail.count })}</small>;
  }
}

function ItemRow({ item, on }: { readonly item: TreeItem; readonly on: TreeIntents }) {
  const { t } = useTranslation();
  const format = useFormat();
  const tag = item.tag;
  const unmovable = tag?.kind === 'unmovable' && !tag.forced;
  return (
    <TreeRow
      depth={1}
      off={unmovable}
      check={
        unmovable ? (
          <LockedCheck />
        ) : (
          <Check
            value={item.picked}
            label={item.label}
            onChange={(picked) => {
              on.onToggleItem(item.key, picked);
            }}
          />
        )
      }
      name={item.label}
      details={
        <>
          <Detail detail={item.detail} />
          {unmovable && (
            <small>
              {tag.reason}{' '}
              <LinkButton
                onClick={() => {
                  on.onIncludeAnyway(item.key);
                }}
              >
                {t('send.customize.includeAnyway')}
              </LinkButton>
            </small>
          )}
        </>
      }
      meta={
        <>
          {tag?.kind === 'sensitive' && <Badge tone="warning">{t('badge.sensitive')}</Badge>}
          {tag?.kind === 'suggested' && <Badge>{t('badge.suggested')}</Badge>}
          {format.bytes(item.bytes)}
        </>
      }
    />
  );
}

function PathRow({
  item,
  path,
  on,
}: {
  readonly item: TreeItem;
  readonly path: TreePath;
  readonly on: TreeIntents;
}) {
  return (
    <TreeRow
      depth={2}
      off={!item.picked}
      check={
        <Check
          value={item.picked && path.picked}
          label={path.display}
          onChange={(picked) => {
            on.onTogglePath(item.key, path.path, picked);
          }}
        />
      }
      name={<Path>{path.display}</Path>}
    />
  );
}

/** An item and, below it, each of its declared paths. */
function itemRows(item: TreeItem, on: TreeIntents) {
  return [
    <ItemRow key={item.key} item={item} on={on} />,
    ...item.paths.map((path) => (
      <PathRow key={`${item.key}|${path.path}`} item={item} path={path} on={on} />
    )),
  ];
}

/** Apps → items → paths, single files one level down. Unmovable items stay visible with their reason. */
export function CustomizeTree({
  apps,
  on,
}: {
  readonly apps: readonly TreeApp[];
  readonly on: TreeIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Tree label={t('send.customize.tree')}>
      {apps.flatMap((app) => [
        <TreeRow
          key={app.id}
          depth={0}
          expanded={app.expanded}
          onToggleExpanded={() => {
            on.onExpand(app.id);
          }}
          check={
            <Check
              value={appCheck(app)}
              label={app.name}
              onChange={(picked) => {
                on.onToggleApp(app.id, picked);
              }}
            />
          }
          name={app.name}
          meta={format.bytes(app.bytes)}
        />,
        ...(app.expanded ? app.items.flatMap((item) => itemRows(item, on)) : []),
      ])}
    </Tree>
  );
}
