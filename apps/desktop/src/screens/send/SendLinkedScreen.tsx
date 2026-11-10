import { useTranslation } from 'react-i18next';
import type { OsKind } from '../../bindings/OsKind';
import { ActionBar, Banner, Button, FlowLayout, Handshake, Head, Note } from '../../ui';
import type { LinkedIntents, LinkedModel } from './model';
import { manifestEnd, osName, RouteChip, SendRail } from './SendRail';

/**
 * Paired: the handshake lands and the receiver reviews the plan. Moving to
 * another OS says so up front, since the receiver will see changed paths.
 */
export function SendLinkedScreen({
  model,
  on,
}: {
  readonly model: LinkedModel;
  readonly on: LinkedIntents;
}) {
  const { t } = useTranslation();
  const { here, peer } = model;
  const peerEnd = peer.os === null ? null : { name: peer.name, os: peer.os };
  const os = (kind: OsKind | null) => (kind === null ? null : { kind, label: osName(kind, t) });
  const peerOs = os(peer.os);
  return (
    <FlowLayout
      route={peerEnd && <RouteChip here={here} to={peerEnd} />}
      {...(model.facts !== null && {
        rail: (
          <SendRail
            move={model}
            purpose="send"
            to={
              peerEnd === null
                ? { name: peer.name, note: t('send.linked.reviewing') }
                : manifestEnd(peerEnd, t('send.linked.reviewing'), t)
            }
          />
        ),
      })}
      actions={
        <ActionBar
          start={
            <Button kind="quiet" onClick={on.onCancel}>
              {t('action.cancel')}
            </Button>
          }
          note={t('send.linked.waitingFor', { peer: peer.name })}
          end={
            <Button kind="primary" busy disabled>
              {t('send.linked.waiting')}
            </Button>
          }
        />
      }
    >
      <Head
        center
        title={t('send.linked.title', { peer: peer.name })}
        lede={t('send.linked.lede')}
      />
      <Handshake
        here={here?.name ?? t('send.live.here')}
        there={peer.name}
        linked
        hereOs={os(here?.os ?? null)}
        thereOs={peerOs}
      />
      {here !== null && peerOs !== null && here.os !== peerOs.kind && (
        <Banner
          tone="accent"
          icon="swap"
          title={t('send.linked.crossOsTitle')}
          body={t('send.linked.crossOsBody', { peer: peer.name, os: peerOs.label })}
        />
      )}
      {model.replaced > 0 && (
        <Note center>{t('send.live.replaced', { count: model.replaced })}</Note>
      )}
    </FlowLayout>
  );
}
