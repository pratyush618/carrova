import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { PickRejection } from '../../bindings/PickRejection';
import { useFormat } from '../../i18n/useFormat';
import {
  Button,
  Check,
  GroupHead,
  Icon,
  IconButton,
  List,
  Note,
  Path,
  Row,
  type IconName,
} from '../../ui';
import type { FoundIntents, FoundModel, ManualPick } from './model';

type ManualModel = Extract<FoundModel, { status: 'manual' }>;
type ManualIntents = Pick<
  FoundIntents,
  'onAddFiles' | 'onAddFolders' | 'onRemovePick' | 'onConfirmKey' | 'onLeaveKey'
>;

const KIND_ICON: Readonly<Record<ManualPick['kind'], IconName>> = {
  file: 'file',
  dir: 'folder',
  symlink: 'arrow',
};

/** Why the engine refused a path, in the same words as the CLI. */
export function rejectionText(reason: PickRejection, t: TFunction): string {
  switch (reason.kind) {
    case 'is_root':
    case 'contains_root':
      return t(`send.manual.rejected.${reason.kind}`, {
        root: t(`send.manual.roots.${reason.root}`),
      });
    case 'unreadable':
      return t('send.manual.rejected.unreadable', { reason: reason.reason });
    default:
      return t(`send.manual.rejected.${reason.kind}`);
  }
}

function PickLine({ pick, on }: { readonly pick: ManualPick; readonly on: ManualIntents }) {
  const { t } = useTranslation();
  const format = useFormat();
  const size = format.bytes(pick.bytes);
  const remove = on.onRemovePick && (
    <IconButton
      icon="close"
      label={t('send.manual.remove', { path: pick.path })}
      onClick={() => {
        on.onRemovePick?.(pick.key);
      }}
    />
  );
  const sub =
    pick.kind === 'dir'
      ? `${size} · ${t('count.files', { count: pick.files })}`
      : pick.kind === 'symlink'
        ? t('send.manual.link')
        : size;
  if (!pick.sensitive) {
    return (
      <Row
        lead={<Icon name={KIND_ICON[pick.kind]} />}
        title={<Path>{pick.path}</Path>}
        sub={sub}
        trail={remove}
      />
    );
  }
  return (
    <Row
      off={!pick.included}
      lead={
        <>
          <Check
            warn
            value={pick.included}
            label={t('send.found.bring', { app: pick.path })}
            onChange={(bring) => {
              if (bring) on.onConfirmKey(pick.key);
              else on.onLeaveKey(pick.key);
            }}
          />
          <Icon name="key" />
        </>
      }
      title={<Path>{pick.path}</Path>}
      sub={sub}
      trail={
        <>
          {!pick.included && (
            <Button
              size="sm"
              aria-label={t('send.found.includeKey', { item: pick.path })}
              onClick={() => {
                on.onConfirmKey(pick.key);
              }}
            >
              {t('send.found.include')}
            </Button>
          )}
          {remove}
        </>
      }
    />
  );
}

/**
 * Manual mode's body: the two system pickers, then the picks (secrets apart,
 * each asked first), paths still being measured, and what was refused and why.
 */
export function ManualPicks({
  model,
  on,
}: {
  readonly model: ManualModel;
  readonly on: ManualIntents;
}) {
  const { t } = useTranslation();
  const secrets = model.picks.filter((p) => p.sensitive);
  const plain = model.picks.filter((p) => !p.sensitive);
  const empty = model.picks.length === 0 && model.measuring.length === 0;
  return (
    <>
      {(on.onAddFiles ?? on.onAddFolders) && (
        <div className="s-manual-add">
          {on.onAddFiles && (
            <Button icon="file" onClick={on.onAddFiles}>
              {t('send.manual.addFiles')}
            </Button>
          )}
          {on.onAddFolders && (
            <Button icon="folder" onClick={on.onAddFolders}>
              {t('send.manual.addFolder')}
            </Button>
          )}
        </div>
      )}
      {empty && <Note center>{t('send.manual.empty')}</Note>}
      {secrets.length > 0 && (
        <List label={t('send.found.secrets')} warn={t('send.manual.sensitive')}>
          {secrets.map((pick) => (
            <PickLine key={pick.key} pick={pick} on={on} />
          ))}
        </List>
      )}
      {plain.length + model.measuring.length > 0 && (
        <List label={t('send.manual.list')}>
          {plain.map((pick) => (
            <PickLine key={pick.key} pick={pick} on={on} />
          ))}
          {model.measuring.map((path) => (
            <Row
              key={path}
              off
              lead={<Icon name="clock" />}
              title={<Path>{path}</Path>}
              sub={t('send.manual.measuring')}
            />
          ))}
        </List>
      )}
      {model.absorbed.map(({ path, into }) => (
        <Note key={path}>{t('send.manual.absorbed', { path, into })}</Note>
      ))}
      {model.rejected.length > 0 && (
        <section>
          <GroupHead title={t('send.manual.rejectedList')} />
          <List label={t('send.manual.rejectedList')}>
            {model.rejected.map(({ path, reason }) => (
              <Row
                key={path}
                lead={<Icon name="warn" />}
                title={<Path>{path}</Path>}
                sub={rejectionText(reason, t)}
              />
            ))}
          </List>
        </section>
      )}
    </>
  );
}
